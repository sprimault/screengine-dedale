// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le temps qu'une image coûte, et où il part.
//!
//! **Une cadence ne mesure rien tant que le budget tient.** La boucle de l'étage
//! d'accueil joue à pas fixe et ne rend qu'une image par réveil qui a fait avancer
//! la partie : le compte d'images reste donc collé au taux de pas, et il y reste
//! quand un lot double son coût. Ce qui bouge avant le décrochage est le **temps**,
//! et la cadence ne dit plus que le moment où il a cédé.
//!
//! **Et le temps se décompose, sinon il ne dit pas qui a payé.** Un rayon de
//! détection par créature tombe dans le pas de jeu, un sprite de plus dans la
//! soumission : un total qui monte sans nommer sa part se rediagnostique de zéro.
//!
//! **Trois des quatre parts viennent de l'étage d'accueil**, qui les borne là où le
//! jeu n'a pas de prise : le remplissage des tuiles, et l'écran jusqu'à la
//! présentation — laquelle porte la mise à l'échelle, donc le coût d'une fenêtre
//! agrandie d'un facteur de trop. La part du pas de jeu reste ici, parce que rien
//! d'autre ne la voit.
//!
//! **Sur une durée et non sur un nombre d'images.** À nombre d'images fixe, le
//! relevé ralentit à mesure que l'image coûte, c'est-à-dire au moment précis où on
//! le regarde.
//!
//! **Le pire avec la moyenne.** Un à-coup est ce qui se voit en jouant, et c'est
//! exactement ce qu'une moyenne sur une seconde efface.
//!
//! **Rien dans le tampon de sortie**, et c'est la frontière du module : un chiffre
//! dessiné à l'écran serait de l'interface, donc un sujet à part. La barre de titre
//! est là où un relevé de développement se lit sans entrer dans l'image.

use std::fmt;
use std::time::{Duration, Instant};

use screengine_play::FrameTime;

#[cfg(test)]
mod tests;

/// La durée d'un relevé.
///
/// Une seconde : assez pour que soixante images se moyennent, assez peu pour qu'une
/// cote se juge en jouant plutôt qu'en attendant.
const SPAN: Duration = Duration::from_secs(1);

/// Au-delà de cette durée, un relevé a été interrompu et ne se publie pas.
///
/// La boucle attend sans rendre quand la fenêtre est masquée, et jette son temps
/// accumulé après un redimensionnement. Le relevé qui enjambe l'un ou l'autre
/// compte ses images sur une durée qu'elles n'ont pas occupée : il rendrait une
/// cadence effondrée sans que rien n'ait coûté plus cher, ce qui est le pire état
/// d'un instrument.
///
/// **Distincte de [`SPAN`] malgré le voisinage des deux valeurs** : l'une dit quand
/// publier, l'autre quand se taire. Les fondre ferait d'un relevé un peu tardif un
/// relevé interrompu.
const STALE: Duration = Duration::from_secs(2);

/// Le cumul d'un relevé, sans horloge.
///
/// Séparé de [`Pace`] pour être éprouvé : celui-ci lit l'horloge, qu'aucune épreuve
/// ne peut reposer, là où ce cumul n'est qu'une fonction des durées qu'on lui verse.
#[derive(Default)]
struct Counts {
    /// Les images closes.
    frames: u32,
    /// Le cumul de ce qu'une image a coûté, pas de jeu compris.
    total: Duration,
    /// La plus chère des images.
    peak: Duration,
    /// Le cumul des pas de jeu.
    game: Duration,
    /// Le cumul des soumissions.
    scene: Duration,
    /// Le cumul des remplissages de tuiles.
    engine: Duration,
    /// Le cumul de l'écran : l'interface, la mise à l'échelle et la présentation.
    screen: Duration,
}

impl Counts {
    /// Verse une image, en quatre parts disjointes.
    ///
    /// Disjointes à la source, et c'est ce qui rend la somme lisible : l'étage
    /// d'accueil borne les trois dernières bout à bout sur la même image, et aucune
    /// soustraction n'a lieu ici.
    fn image(&mut self, game: Duration, scene: Duration, engine: Duration, screen: Duration) {
        let spent = game + scene + engine + screen;
        self.frames += 1;
        self.total += spent;
        self.peak = self.peak.max(spent);
        self.game += game;
        self.scene += scene;
        self.engine += engine;
        self.screen += screen;
    }

    /// Ce que ce cumul rend sur la durée écoulée, ou rien quand il n'y a pas de
    /// relevé à publier.
    ///
    /// Rien dans trois cas : aucune image, une durée au-delà de [`STALE`] qui dit
    /// que la boucle s'est arrêtée entre-temps, et une durée nulle — qui ne porte
    /// aucun relevé, et dont le seul effet serait de rendre une cadence infinie.
    fn tally(&self, elapsed: Duration) -> Option<Tally> {
        if self.frames == 0 || elapsed.is_zero() || elapsed > STALE {
            return None;
        }
        Some(Tally {
            rate: self.frames as f32 / elapsed.as_secs_f32(),
            image: self.total / self.frames,
            peak: self.peak,
            game: self.game / self.frames,
            scene: self.scene / self.frames,
            engine: self.engine / self.frames,
            screen: self.screen / self.frames,
        })
    }
}

/// Ce qu'un relevé clos a retenu, par image.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Tally {
    /// Les images par seconde.
    rate: f32,
    /// Ce qu'une image a coûté en moyenne, de son pas de jeu à son interface.
    image: Duration,
    /// Ce qu'a coûté la plus chère.
    peak: Duration,
    /// La part du pas de jeu.
    game: Duration,
    /// La part de la soumission.
    scene: Duration,
    /// La part du remplissage des tuiles.
    engine: Duration,
    /// La part de l'écran, présentation comprise.
    screen: Duration,
}

impl fmt::Display for Tally {
    /// Un relevé d'une ligne, pour la barre de titre.
    ///
    /// Les quatre parts somment à la moyenne, et c'est ce qui se vérifie d'un coup
    /// d'œil avant de croire un chiffre.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /// Une durée en millisecondes, l'unité où une part d'image se lit.
        fn ms(spent: Duration) -> f32 {
            spent.as_secs_f32() * 1000.0
        }

        write!(
            f,
            "{:.0} i/s, {:.1} ms (pire {:.1}), jeu {:.1}, scène {:.1}, \
             moteur {:.1}, écran {:.1}",
            self.rate,
            ms(self.image),
            ms(self.peak),
            ms(self.game),
            ms(self.scene),
            ms(self.engine),
            ms(self.screen)
        )
    }
}

/// L'instrument : ce qu'une image coûte, relevé seconde par seconde.
///
/// **Ni du monde ni de la partie**, comme le relevé de visibilité : il ne décide de
/// rien, survit au rechargement de la carte comme à une course perdue, et la boucle
/// seule l'alimente.
///
/// Trois appels par image, dans l'ordre où la boucle les atteint : [`Pace::stepped`]
/// à chaque pas de jeu, [`Pace::rendered`] en queue du rappel de rendu, et
/// [`Pace::timed`] au pas suivant, quand l'étage d'accueil rend ce que l'image a
/// coûté. Les sauter ne fait rien perdre d'autre que le relevé.
pub struct Pace {
    /// Le début du relevé en cours.
    since: Instant,
    /// Ce que les pas de jeu ont coûté depuis la dernière image soumise.
    ///
    /// **Rattaché à l'image qui suit et non au relevé** : un réveil tardif joue
    /// plusieurs pas pour une seule image, et c'est le budget d'une image que la
    /// somme doit dire.
    stepping: Duration,
    /// Les pas de l'image soumise, en attente de ce qu'elle a coûté.
    ///
    /// **C'est aussi ce qui la compte une fois et une seule.** L'étage d'accueil rend
    /// les durées de la dernière image rendue, et plusieurs pas se jouent pour une
    /// seule image quand la machine prend du retard : ils lisent alors tous la même
    /// valeur, et la verser à chaque fois gonflerait le compte d'images sans gonfler
    /// le temps — donc une cadence inventée. Ici, une image ne se verse que si un
    /// rendu l'a annoncée.
    pending: Option<Duration>,
    /// Le cumul du relevé en cours.
    counts: Counts,
    /// Ce que le dernier relevé publié a dit.
    taken: Option<Tally>,
}

impl Pace {
    /// Un instrument qui ouvre son premier relevé maintenant.
    pub fn new() -> Self {
        Self {
            since: Instant::now(),
            stepping: Duration::ZERO,
            pending: None,
            counts: Counts::default(),
            taken: None,
        }
    }

    /// Ajoute ce qu'un pas de jeu a coûté.
    ///
    /// La seule des trois méthodes à prendre une durée mesurée ici : le pas de jeu
    /// est le seul morceau que l'étage d'accueil ne borne pas, et il peut arriver
    /// plusieurs fois avant une image.
    pub fn stepped(&mut self, spent: Duration) {
        self.stepping += spent;
    }

    /// Annonce une image, en queue du rappel de rendu.
    ///
    /// Elle n'a encore coûté que ses pas de jeu ; le reste arrive au pas suivant,
    /// par [`Pace::timed`].
    pub fn rendered(&mut self) {
        self.pending = Some(std::mem::take(&mut self.stepping));
    }

    /// Verse ce que l'image annoncée a coûté, et publie le relevé s'il est échu.
    ///
    /// Sans effet quand aucun rendu n'attend ses durées : la même valeur se relit à
    /// chaque pas, et c'est l'annonce qui dit laquelle est neuve.
    pub fn timed(&mut self, frame: FrameTime) {
        let Some(game) = self.pending.take() else {
            return;
        };
        self.counts
            .image(game, frame.submit, frame.fill, frame.screen);

        let elapsed = self.since.elapsed();
        if elapsed >= SPAN {
            // Un relevé interrompu laisse en place celui d'avant plutôt que de vider
            // la barre de titre : il ne s'est rien passé qu'on puisse lire, et une
            // attente qui revient au retour d'une fenêtre masquée se lirait comme une
            // panne de l'instrument.
            if let Some(tally) = self.counts.tally(elapsed) {
                self.taken = Some(tally);
            }
            self.counts = Counts::default();
            self.since = Instant::now();
        }
    }
}

impl fmt::Display for Pace {
    /// Le dernier relevé publié, ou l'attente du premier.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.taken {
            Some(tally) => tally.fmt(f),
            None => f.write_str("relevé en cours"),
        }
    }
}
