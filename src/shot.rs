// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le rayon contre le décor : d'où il part, ce qu'il rencontre, et ce qu'on peut
//! en lire.
//!
//! **Un tir se résout en deux temps, et seul le premier est ici.** Le rayon contre
//! la géométrie de cellule est l'affaire du moteur, qui rend une fraction, une
//! normale et un point de contact ; le test contre le volume d'un monstre est
//! l'affaire du jeu, qui a placé ce volume. Le second n'a pas de portail, donc pas
//! d'adjacence : rien de ce que le balayage traverse ne s'applique à lui.
//!
//! **Il ne lit ni caméra, ni horloge, ni entrée.** Une origine, une direction, une
//! cellule : le tir est une fonction d'entrées vers des sorties, donc il s'éprouve
//! sans ouvrir de fenêtre et sans reposer une pose de caméra. C'est l'appelant qui
//! compose la direction du regard, parce que c'est lui qui tient la caméra.
//!
//! **Et il ne lit pas tout ce que le moteur lui rend.** Le point de contact n'est
//! valable que si le trajet a été examiné en entier — [`Shot::impact`] dit pourquoi,
//! et c'est la seule subtilité du module.

use screengine_play::{Hit, Surfaces, Vec3, World};

#[cfg(test)]
mod tests;

/// La portée d'un tir, en unités de monde.
///
/// **Trente-deux, soit huit cases, et la borne se justifie par ce qu'on
/// distingue** : à la résolution interne basse, un démon à cette distance fait une
/// poignée de pixels, et aucun réticule ne vise. Tirer plus loin serait tirer sur ce
/// qu'on ne voit pas.
///
/// **Elle tient aussi la troncature à distance**, et c'est son second effet : la
/// borne du balayage se consomme en **cellules** et non en distance, et le trajet qui
/// la sature est le trajet dégagé — exactement un tir le long d'un couloir. Huit
/// cases en restent très loin, même en comptant les paliers et les cages qu'un rayon
/// traverse en chemin.
pub const RANGE: f32 = 32.0;

/// Un tir, et ce que le décor lui a opposé.
///
/// **Le trajet demandé est gardé avec le résultat**, et ce n'est pas de la
/// commodité : la fraction que le moteur rend est relative à ce trajet, donc elle ne
/// veut rien dire sans lui. C'est aussi ce qui permettra à l'étape suivante de
/// comparer cette fraction à celle d'un volume de monstre — à condition que les deux
/// portent sur le même segment.
#[derive(Clone, Copy)]
pub struct Shot {
    /// D'où le rayon part : l'œil, en coordonnées de monde.
    pub from: Vec3,
    /// Où il s'arrête faute d'avoir rencontré quoi que ce soit, donc à sa portée.
    pub to: Vec3,
    /// La cellule donnée au balayage.
    pub cell: u32,
    /// Ce que le décor a opposé, ou rien si le tir est parti de nulle part.
    pub hit: Option<Hit>,
}

impl Shot {
    /// Vrai si le décor a arrêté le rayon avant sa portée.
    ///
    /// **Le critère est la fraction, jamais l'identifiant de surface.** Celui-ci
    /// vaut zéro pour trois cas distincts dont deux arrêtent : un portail non
    /// apparié, qui est un mur comme un autre, et un trajet tronqué. C'est la clause
    /// que la politique de collision tient déjà pour le balayage d'un corps, et le
    /// tir n'a aucune raison de la lire autrement.
    pub fn blocked(&self) -> bool {
        self.hit.is_some_and(|hit| hit.fraction < 1.0)
    }

    /// Où le rayon a touché le décor, quand on peut le savoir.
    ///
    /// **Rien quand le trajet a été tronqué**, et c'est tout l'intérêt de la
    /// méthode. Le moteur borne la région qu'il examine à un nombre de cellules ;
    /// au-delà, il recule la fraction, efface la surface et la normale — réponse
    /// conservatrice, puisque déclarer le trajet libre ferait traverser un mur qu'il
    /// n'a pas regardé. Mais **il laisse le point de contact tel qu'il l'avait
    /// initialisé**, c'est-à-dire au bout du trajet demandé : une marque posée là se
    /// poserait au-delà de ce qui a été examiné.
    ///
    /// La portée du tir rend ce cas inatteignable en jeu. La méthode ne s'y fie pas
    /// pour autant : c'est une coïncidence de réglages, pas une garantie, et elle
    /// cesserait de tenir le jour où la portée bougerait.
    pub fn impact(&self) -> Option<Vec3> {
        self.hit
            .filter(|hit| hit.fraction < 1.0 && !hit.incomplete)
            .map(|hit| hit.point)
    }
}

/// Tire un rayon depuis l'œil, dans la direction du regard.
///
/// `ahead` est attendu **unitaire** : il vient d'une orientation de caméra, qui en
/// rend toujours un. La portée s'applique telle quelle, donc une direction plus
/// courte raccourcirait le tir sans que rien ne le dise.
///
/// **Une cellule nulle ne tire pas**, et le partage se fait ici parce que le chemin
/// Rust du moteur ne le fait pas : sa méthode de rayon rend l'absence aussi bien pour
/// une cellule nulle que pour une cellule inconnue, là où sa frontière C en fait deux
/// cas distincts — déplacement libre d'un côté, erreur de l'autre. Zéro veut dire
/// « hors de tout volume », et un tir parti de là n'a aucun décor à rencontrer.
///
/// **Le filtre prend les surfaces solides seules.** Le décor engendré n'en écrit
/// aujourd'hui aucune autre, donc les deux filtres y voient la même chose ; celui-ci
/// est celui qui restera juste le jour où une grille ou une vitre apparaîtra, qu'un
/// tir doit traverser et qu'une sélection d'éditeur doit attraper.
pub fn fire(map: &World, from: Vec3, ahead: Vec3, cell: u32) -> Shot {
    let to = from + ahead * RANGE;
    Shot {
        from,
        to,
        cell,
        hit: match cell {
            0 => None,
            _ => map.pick(cell, from, to, Surfaces::Solid),
        },
    }
}
