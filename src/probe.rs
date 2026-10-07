// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le relevé de ce que la traversée rend, dans un fichier.
//!
//! **Un scintillement ne se lit pas au vol.** Une portion de décor qui disparaît
//! le temps de quelques images ne se laisse ni lire dans un titre, ni saisir dans
//! une capture : il faut qu'elle s'écrive quelque part. Ce module l'écrit.
//!
//! **Et il ne note que les changements d'état.** Une ligne par image en donnerait
//! des milliers par minute, donc rien de lisible : une ligne quand la visibilité
//! cesse d'être complète, une quand elle le redevient, avec le nombre d'images que
//! cela a duré.
//!
//! **Hors dépôt**, dans `.tmp/`, que l'antivirus ne surveille pas et que git
//! ignore — c'est le répertoire où tout ce que ce projet écrit doit aller.

use std::fmt;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use screengine_play::{FreeCamera, Output, Vec3, Visibility, World};

use crate::shot::{RANGE, Shot};

#[cfg(test)]
mod tests;

/// Où le relevé s'écrit.
const LOG: &str = ".tmp/visibilite.log";

/// La proportion de points d'une image qui diffèrent du fond, en pourcentage.
///
/// **Ce qui diffère du fond a été peint, et le fond n'est pas forcément noir** : le
/// contrat du moteur dit qu'un pixel qu'aucun triangle n'a peint est infiniment
/// lointain, donc qu'il prend la couleur du brouillard de lui-même. Comparer au noir
/// rendrait ce relevé muet dès qu'un brouillard serait réglé — et muet sans rien
/// dire, ce qui est le pire état d'un instrument.
///
/// **Une grille de points, pas tous les pixels** : un rendu logiciel coûte déjà
/// cher, et deux cent cinquante-six sondes suffisent — une image de décor n'a pas de
/// raison de tomber sur le fond partout sauf en ces points-là.
///
/// **La proportion, et non le tout ou rien** : une image coupée en deux par une
/// verticale est aussi anormale qu'une image vide, et bien plus probable — un sprite
/// dessiné au-delà de la coupure suffit à ce qu'un seuil « entièrement noir » ne la
/// voie jamais.
///
/// Séparée du relevé pour être éprouvée : celui-ci ouvre un fichier et localise une
/// cellule, là où ce compte n'est qu'une fonction du tampon.
fn painted(output: &mut Output<'_>, background: [u8; 3]) -> u32 {
    /// Les sondes par axe.
    const GRID: u32 = 16;

    let (width, height) = (output.width(), output.height());
    let mut lit = 0;
    for row in 0..GRID {
        for column in 0..GRID {
            let x = column * width / GRID;
            let y = row * height / GRID;
            if let Some(pixel) = output.pixel(x, y) {
                if pixel[..3] != background {
                    lit += 1;
                }
            }
        }
    }
    lit * 100 / (GRID * GRID)
}

/// La pose que le relevé situe, et de quoi la rejouer.
///
/// **Sans l'orientation, un épisode n'est pas reproductible** : on sait qu'une
/// image s'est vidée et où, jamais en regardant quoi. Dix-huit épisodes relevés
/// en marchant n'ont servi à rien pour cette raison — aucune épreuve ne pouvait
/// reposer la caméra là où elle était.
///
/// **Le lacet et le tangage, et non le quaternion rendu** : ce sont eux que
/// `FreeCamera` tient pour état réel et dont elle recompose son orientation à
/// chaque image. Reposés tels quels dans une `FreeCamera`, ils rendent la pose
/// au bit près, sans convention d'ordre de composition à redeviner.
#[derive(Clone, Copy)]
pub struct Aim {
    /// Où l'œil est, en coordonnées de monde.
    pub eye: Vec3,
    /// Le lacet, en radians, comme la caméra libre le porte.
    pub yaw: f32,
    /// Le tangage, en radians, de même.
    pub pitch: f32,
    /// La cellule donnée à la traversée, qui n'est pas forcément celle de l'œil.
    pub cell: u32,
}

impl Aim {
    /// Ce que le jeu regarde, pris de sa caméra libre.
    pub fn new(camera: &FreeCamera, cell: u32) -> Self {
        Self {
            eye: camera.position,
            yaw: camera.yaw,
            pitch: camera.pitch,
            cell,
        }
    }
}

impl fmt::Display for Aim {
    /// La queue commune des trois sortes de lignes.
    ///
    /// **Les angles en degrés** : on les relit à l'œil pour se situer dans un
    /// couloir, et un quart de tour en radians ne se reconnaît pas. La précision
    /// au centième suffit à reposer la pose — le défaut qu'on traque tient à des
    /// centièmes d'unité, pas à des millièmes de degré.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "œil ({:.2} {:.2} {:.2}), lacet {:.2}°, tangage {:.2}°, cellule {}",
            self.eye.x,
            self.eye.y,
            self.eye.z,
            self.yaw.to_degrees(),
            self.pitch.to_degrees(),
            self.cell
        )
    }
}

/// Ce qu'une soumission de scène a donné.
///
/// **Quatre états et non trois** : la visibilité du moteur en décrit trois, et le
/// quatrième est le refus, qui ne lui appartient pas — il vient de la capacité de
/// triangles du contexte, et il **annule la soumission du décor entier**. C'est
/// donc le seul des quatre qui donne une image noire complète, et il doit se
/// distinguer des autres.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    /// La scène a été soumise, et le moteur dit ce qu'elle montre.
    Shown(Visibility),
    /// La soumission a été refusée, donc rien n'a été soumis.
    Refused,
}

impl State {
    /// Comment la ligne du relevé la nomme.
    fn label(self) -> String {
        match self {
            Self::Shown(seen) => format!("{seen:?}"),
            Self::Refused => String::from("Refused — décor entier non soumis"),
        }
    }
}

/// Une soumission dont le seul refus se relève, le décor mis à part.
///
/// **Le décor n'en fait pas partie**, et c'est ce qui justifie deux chemins : sa
/// soumission rend une visibilité en plus de pouvoir échouer, donc elle a quatre
/// états là où celles-ci en ont deux. Les fondre obligerait à porter un
/// `Visibility` qui n'aurait pas de sens pour une arme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Part {
    /// L'arme en main.
    Weapon,
    /// Les créatures du décor.
    Monsters,
}

/// Combien de parties [`Part`] nomme, donc la taille du relevé qui les suit.
const PARTS: usize = 2;

impl Part {
    /// Comment la ligne du relevé la nomme.
    fn label(self) -> &'static str {
        match self {
            Self::Weapon => "l'arme",
            Self::Monsters => "les créatures",
        }
    }
}

/// Ce que le relevé garde entre deux images.
///
/// **Un état de partie**, comme le reste : il est jeté au rechargement de la
/// carte, et il ne regarde que ce que le rendu vient de rendre.
pub struct Probe {
    /// Le fichier, ou rien si on n'a pas pu l'ouvrir.
    ///
    /// **Un relevé qui échoue ne doit pas arrêter le jeu** : il sert à diagnostiquer
    /// une image, pas à la produire. L'échec se dit une fois sur la sortie
    /// d'erreur, et la partie continue sans lui.
    file: Option<File>,
    /// Ce que la soumission a rendu la dernière fois.
    last: State,
    /// Depuis combien d'images elle rend la même chose.
    runs: u64,
    /// L'index d'image, qui situe une ligne dans le temps.
    frame: u64,
    /// Vrai si la dernière image échantillonnée était noire.
    dark: bool,
    /// Pour chaque partie de [`Part`], vrai si sa dernière soumission a été
    /// refusée.
    refused: [bool; PARTS],
}

impl Probe {
    /// Ouvre le relevé, en écrasant celui de la partie précédente.
    ///
    /// **Écrasé et non prolongé** : un fichier qui s'accumule d'un lancement à
    /// l'autre fait chercher dans quelle partie une ligne a été écrite, ce qui est
    /// exactement ce qu'on veut éviter.
    pub fn new() -> Self {
        let file = Path::new(LOG)
            .parent()
            .map(fs::create_dir_all)
            .transpose()
            .and_then(|_| File::create(LOG).map(Some))
            .unwrap_or_else(|erreur| {
                eprintln!("relevé de visibilité indisponible : {erreur}");
                None
            });

        Self {
            file,
            last: State::Shown(Visibility::Complete),
            runs: 0,
            frame: 0,
            dark: false,
            refused: [false; PARTS],
        }
    }

    /// Note si l'image finie est noire, en l'échantillonnant.
    ///
    /// **C'est la mesure que rien d'autre ne donne.** Le moteur n'expose aucun
    /// compte de triangles préparés, donc une soumission qui réussit et rend
    /// `Complete` ne dit pas si l'image porte quelque chose. Le tampon de sortie,
    /// lui, contient l'image finie : il suffit de la regarder.
    ///
    /// **Une grille de points, pas tous les pixels** : un rendu logiciel coûte
    /// déjà cher, et deux cent cinquante-six sondes suffisent — une image de décor
    /// n'a pas de raison d'être noire partout sauf en ces points-là. Le plan de
    /// contrôle n'est pas encore dessiné quand on mesure, donc il ne compte pas
    /// comme un pixel peint.
    pub fn look(&mut self, output: &mut Output<'_>, map: &World, aim: &Aim, background: [u8; 3]) {
        /// En deçà de ce pourcentage de points peints, l'image est amputée.
        ///
        /// **Quatre-vingt-dix et non cent** : une image de couloir laisse
        /// légitimement quelques sondes sur le fond — un angle sombre, une
        /// embrasure. Une coupure franche, elle, en emporte des dizaines.
        const THRESHOLD: u32 = 90;

        let (width, height) = (output.width(), output.height());
        if width == 0 || height == 0 {
            return;
        }

        let painted = painted(output, background);
        let dark = painted < THRESHOLD;
        if dark == self.dark {
            return;
        }
        self.dark = dark;

        let Some(file) = self.file.as_mut() else {
            return;
        };
        // **La localisation ne se paie que sur une image fautive**, et jamais aux
        // mille autres : elle parcourt toutes les cellules et toutes leurs faces.
        // Ce qu'elle dit est décisif — si elle diffère de la cellule donnée au
        // rendu, la traversée est partie d'un endroit où la caméra n'est pas, et
        // c'est au jeu de le corriger.
        let found = map.locate(aim.eye);
        let _ = writeln!(
            file,
            "image {} : image {} à {painted} % — taille {width}×{height}, \
             {aim}, œil dans {found}{}",
            self.frame,
            if dark { "amputée" } else { "revenue" },
            if found == aim.cell {
                ""
            } else {
                " — DIVERGENCE"
            }
        );
    }

    /// Note si la soumission de cette partie a été refusée, ou qu'elle ne l'est
    /// plus.
    ///
    /// **Le refus du décor se relève, celui du reste se perdait.** Toutes les
    /// soumissions peuvent échouer pour la même raison — la capacité de triangles
    /// —, et l'arme disparaîtrait de la main, les créatures du couloir, sans
    /// qu'aucune ligne ne le dise.
    ///
    /// **Une fonction et non une par partie** : c'est le troisième corps identique
    /// à un libellé près, et le moment où le motif se nomme plutôt que de se
    /// recopier une fois de plus. Le décor garde la sienne, qui relève une
    /// visibilité en plus d'un refus.
    ///
    /// Relevé au changement comme le reste : une capacité dépassée l'est pendant des
    /// dizaines d'images, et une ligne par image noierait le relevé.
    pub fn refusal(&mut self, part: Part, refused: bool, aim: &Aim) {
        let was = &mut self.refused[part as usize];
        if refused == *was {
            return;
        }
        *was = refused;

        let Some(file) = self.file.as_mut() else {
            return;
        };
        let _ = writeln!(
            file,
            "image {} : {} {} — {aim}",
            self.frame,
            part.label(),
            if refused {
                "est refusée"
            } else {
                "est revenue"
            }
        );
    }

    /// Note ce que la soumission vient de rendre, si cela a changé.
    ///
    /// **L'erreur compte autant que la visibilité, et c'est elle qu'on cherche** :
    /// un refus ne peut venir que de la capacité de triangles, et un dépassement
    /// **annule la soumission du décor entier** — donc une image noire, et noire
    /// par intermittence puisque le compte dépend de la pose. Ne relever que le cas
    /// qui réussit laisserait passer précisément ce qu'on veut voir.
    ///
    /// La pose et la cellule accompagnent la ligne parce que sans elles on saurait
    /// que le phénomène arrive, pas **où** — et `Incomplete` ne dit pas laquelle
    /// des deux bornes a été atteinte, donc c'est la position qui le dira : à la
    /// borne de profondeur le trou est au loin, à celle des visites il peut être
    /// dans la cellule voisine.
    pub fn note(&mut self, seen: State, aim: &Aim) {
        self.frame += 1;
        if seen == self.last {
            self.runs += 1;
            return;
        }

        let since = self.runs;
        self.last = seen;
        self.runs = 1;

        let Some(file) = self.file.as_mut() else {
            return;
        };
        // L'écriture peut échouer — disque plein, fichier verrouillé —, et le jeu
        // n'a rien à en faire : ce qui compte est qu'il continue de tourner.
        let _ = writeln!(
            file,
            "image {} : {} après {since} image(s) — {aim}",
            self.frame,
            seen.label()
        );
    }

    /// Note ce qu'un tir a rendu, et ce qu'il a visé.
    ///
    /// **Le seul chemin écrit sur appel, et non au changement.** Les trois autres
    /// relèvent des états qui durent des dizaines d'images, donc une ligne par image
    /// les noierait ; un tir est un événement d'un seul pas, et le taire parce qu'il
    /// ressemble au précédent le rendrait invisible.
    ///
    /// **La cellule de départ se relève à côté de celle que la localisation trouve**,
    /// et c'est la raison d'être de la ligne. Le moteur ne vérifie pas que la cellule
    /// donnée contienne le point de départ : lui en passer une fausse rend un
    /// résultat faux **sans aucune erreur**, et rien d'autre ne le dirait. La
    /// localisation parcourt toutes les cellules, mais un tir ne part qu'au clic.
    ///
    /// **L'index est celui de la dernière image rendue**, et la ligne le dit : un tir
    /// se résout dans le pas de mise à jour, donc entre deux images. Le corriger d'un
    /// cran supposerait qu'une image suive, ce que rien ne garantit.
    pub fn fired(&mut self, shot: &Shot, map: &World, aim: &Aim) {
        let Some(file) = self.file.as_mut() else {
            return;
        };

        let found = map.locate(shot.from);
        let what = match shot.hit {
            None => String::from("parti hors de tout volume"),
            _ if !shot.blocked() => format!("rien à {RANGE} de portée"),
            Some(hit) => format!(
                "surface {} dans la cellule {} à {:.3} de portée, normale \
                 ({:.2} {:.2} {:.2}), {}{}",
                hit.surface,
                hit.cell,
                hit.fraction,
                hit.normal.x,
                hit.normal.y,
                hit.normal.z,
                // Le point passe par la méthode qui le refuse sur un trajet tronqué,
                // et non par le champ : c'est là qu'est écrite la raison.
                match shot.impact() {
                    Some(at) => format!("point ({:.2} {:.2} {:.2})", at.x, at.y, at.z),
                    None => String::from("TRONQUÉ, point indisponible"),
                },
                if hit.start_solid {
                    " — DÉPART DANS LE SOLIDE"
                } else {
                    ""
                }
            ),
        };

        let _ = writeln!(
            file,
            "après l'image {} : tir vers ({:.2} {:.2} {:.2}) depuis la cellule {}\
             {} — {what}, {aim}",
            self.frame,
            shot.to.x,
            shot.to.y,
            shot.to.z,
            shot.cell,
            if found == shot.cell {
                String::new()
            } else {
                format!(" — DIVERGENCE, l'œil est dans {found}")
            }
        );
    }

    /// Ce que la soumission rend en ce moment, pour le titre de la fenêtre.
    pub fn seen(&self) -> State {
        self.last
    }
}
