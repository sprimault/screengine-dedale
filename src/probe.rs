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

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use screengine_play::{Output, Vec3, Visibility, World};

/// Où le relevé s'écrit.
const LOG: &str = ".tmp/visibilite.log";

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
    /// Combien de fois le rappel de sortie a été appelé.
    ///
    /// **Comparé au compte des rendus, il dit si une image a été présentée sans
    /// avoir été dessinée** : les deux rappels se suivent par construction, donc
    /// un écart est la signature d'un tampon montré tel quel — noir, avec le seul
    /// plan de contrôle par-dessus.
    shown: u64,
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
            shown: 0,
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
    pub fn look(&mut self, output: &mut Output<'_>, map: &World, eye: Vec3, cell: u32) {
        /// Les sondes par axe.
        const GRID: u32 = 16;

        /// En deçà de ce pourcentage de points peints, l'image est amputée.
        ///
        /// **Quatre-vingt-dix et non cent** : une image de couloir laisse
        /// légitimement quelques sondes sur du noir — un angle sombre, une
        /// embrasure. Une coupure franche, elle, en emporte des dizaines.
        const THRESHOLD: u32 = 90;

        let (width, height) = (output.width(), output.height());
        if width == 0 || height == 0 {
            return;
        }

        let mut lit = 0;
        for row in 0..GRID {
            for column in 0..GRID {
                let x = column * width / GRID;
                let y = row * height / GRID;
                if let Some(pixel) = output.pixel(x, y) {
                    // Le fond est noir ; tout ce qui ne l'est pas a été peint.
                    if pixel[0] | pixel[1] | pixel[2] != 0 {
                        lit += 1;
                    }
                }
            }
        }

        // **La proportion, et non le tout ou rien** : une image coupée en deux par
        // une verticale est aussi anormale qu'une image noire, et bien plus
        // probable — un sprite dessiné au-delà de la coupure suffit à ce qu'un
        // seuil « entièrement noir » ne la voie jamais.
        let painted = lit * 100 / (GRID * GRID);
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
        let found = map.locate(eye);
        let _ = writeln!(
            file,
            "image {} : image {} à {painted} % — taille {width}×{height}, \
             œil ({:.2} {:.2} {:.2}), cellule {cell}, œil dans {found}{}",
            self.frame,
            if dark { "amputée" } else { "revenue" },
            eye.x,
            eye.y,
            eye.z,
            if found == cell { "" } else { " — DIVERGENCE" }
        );
    }

    /// Note qu'une image vient d'être présentée, et signale un rendu manquant.
    ///
    /// **Les deux rappels se suivent**, donc le compte des sorties doit rester égal
    /// à celui des rendus. Un écart veut dire qu'une image a été montrée sans que
    /// la scène ait été soumise : le tampon ne porte alors que le fond et ce que la
    /// sortie y dessine, ce qui est exactement un scintillement noir où le plan de
    /// contrôle reste visible.
    pub fn present(&mut self, eye: Vec3, cell: u32) {
        self.shown += 1;
        if self.shown <= self.frame {
            return;
        }

        let missed = self.shown - self.frame;
        let Some(file) = self.file.as_mut() else {
            return;
        };
        let _ = writeln!(
            file,
            "sortie {} : {missed} image(s) présentée(s) sans rendu — \
             œil ({:.2} {:.2} {:.2}), cellule {cell}",
            self.shown, eye.x, eye.y, eye.z
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
    pub fn note(&mut self, seen: State, eye: Vec3, cell: u32) {
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
            "image {} : {} après {since} image(s) — œil ({:.2} {:.2} {:.2}), cellule {cell}",
            self.frame,
            seen.label(),
            eye.x,
            eye.y,
            eye.z
        );
    }

    /// Ce que la soumission rend en ce moment, pour le titre de la fenêtre.
    pub fn seen(&self) -> State {
        self.last
    }
}
