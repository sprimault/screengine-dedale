// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les tubes au plafond, et leur battement.
//!
//! **Ce qui bat ne se cuit pas, et c'est le partage que le moteur impose.** Une lampe fixe
//! se cuit en lightmap, payée une fois ; un tube qui clignote est une lumière **dynamique**,
//! recalculée par sommet à chaque image. Cuire ce qui bat donnerait une flaque peinte sur
//! le mur, et calculer par sommet ce qui ne bouge pas paierait une atténuation par image
//! pour une valeur constante.
//!
//! **Le moteur n'accepte que huit lumières dynamiques par image**, et c'est une de ses
//! constantes. Le labyrinthe en porte bien davantage, donc chaque image retient **les huit
//! plus proches de l'œil** : au-delà de sa portée un tube n'ajoute rien, et ce qui est
//! écarté est ce qui ne rendait rien.
//!
//! **Un tube éteint est retiré du lot**, et non posé à rayon nul — que le moteur refuse, et
//! qui coûterait de toute façon une atténuation par sommet pour rien.
//!
//! **Le battement suit le temps et non le rang de l'image**, et c'est le seul écart assumé
//! avec l'exemple d'éclairage du moteur. Le sien suit le rang pour qu'une capture se rejoue
//! au bit près d'une machine à l'autre ; ici un battement par image clignoterait deux fois
//! plus vite à cent vingt images par seconde qu'à soixante. Ce dépôt porte déjà un défaut
//! de cette famille — l'inertie du lacet de l'arme, qui vaut un facteur vingt-trois entre
//! trente et cent vingt images —, et il n'y en aura pas un second.

use screengine_play::{Color, Context, CoreError, Light, MAX_LIGHTS, Vec3};

use crate::maze::export::{CEILING, CELL, LEVEL};
use crate::maze::grid::Grid;
use crate::rng::Rng;

#[cfg(test)]
mod tests;

/// Une case sur combien porte un tube.
///
/// **Trois, et ce sont les tubes morts qui font le noir entre deux, pas l'espacement.**
/// C'est la clause de l'exemple du moteur, et elle vaut ici : un décor équipé partout dont
/// les tubes lâchent se lit comme un souterrain mal entretenu, là où des tubes espacés se
/// lisent comme un décor incomplet. La même image, obtenue par ce qui la cause plutôt que
/// par ce qui la simule.
const HUNG: u32 = 3;

/// Ce qui écarte le tirage des tubes de celui des lampes et de celui de la partie.
///
/// Même raison qu'ailleurs : deux générateurs sur une même graine rendent la même suite, et
/// la coïncidence fait rouvrir la question à chaque relecture.
const TUBES: u64 = 0x5455_4245_5300_0000;

/// De combien un tube pend sous le plafond.
const DROP: f32 = 0.15;

/// La portée d'un tube.
///
/// **Un peu plus que le côté d'une case** : deux tubes voisins se rejoignent, de sorte
/// qu'un tube qui lâche laisse une pénombre et non un trou net — lequel trahirait la portée
/// sphérique.
const REACH: f32 = 5.5;

/// Les teintes qu'un tube peut prendre à pleine intensité.
///
/// **La teinte porte aussi l'intensité** : c'est elle qui traverse la frontière, et le
/// moteur n'a pas de facteur à côté. Les quatre sont froides et sourdes — c'est ce que
/// `ROADMAP.md` appelle des sources rares et froides, et ce qui sépare un souterrain d'une
/// cave à la bougie.
const TINTS: [[u8; 3]; 4] = [
    [0x62, 0x86, 0x9E],
    [0x70, 0x8C, 0xA4],
    [0x5C, 0x84, 0xA0],
    [0x6E, 0x92, 0xA8],
];

/// Combien de paliers d'usure le tirage distingue.
const WEARS: u32 = 8;

/// Un tube : sa pose, sa teinte, son décalage de battement et son usure.
pub struct Tube {
    /// Où il pend.
    at: Vec3,
    /// Sa teinte à pleine intensité.
    tint: [u8; 3],
    /// Le décalage de son battement.
    ///
    /// **Sans lui, tout le labyrinthe lâcherait ensemble**, ce qui se lit comme une coupure
    /// de courant et non comme une rangée de tubes fatigués.
    phase: f32,
    /// Son usure, de zéro — un tube qui ne bronche qu'à peine — à un, un tube qui ne
    /// s'allume plus que par saccades.
    wear: f32,
}

/// Les tubes d'un labyrinthe, et le temps qu'ils ont vécu.
///
/// **Du monde et non de la partie** : leurs poses viennent de la grille, donc ils se
/// refont avec la carte. Le temps qu'ils portent est une animation du décor — il avance
/// que le joueur bouge ou non —, et le remettre à zéro en rechargeant est sans conséquence.
pub struct Tubes {
    /// Ce qui pend au plafond.
    items: Vec<Tube>,
    /// Depuis combien de temps ils battent, en secondes.
    time: f32,
}

impl Tubes {
    /// Accroche les tubes d'un labyrinthe.
    ///
    /// **Le générateur vient d'ici et non de l'appelant**, à l'inverse du semis des
    /// créatures : ce qui se tire là-bas se mêle à d'autres tirages de la partie, alors que
    /// les tubes sont du monde et ne tirent rien d'autre. La graine est celle de la grille,
    /// décalée.
    pub fn hang(grid: &Grid) -> Self {
        let (width, height, levels) = grid.extent();
        let mut rng = Rng::new(grid.seed() ^ TUBES);
        let mut items = Vec::new();

        for level in 0..levels {
            for y in 0..height {
                for x in 0..width {
                    // Le tirage est consommé par chaque case, qu'elle porte un tube ou non :
                    // la suite ne dépend alors pas de la topologie, et une graine rejoue le
                    // même plafond.
                    let hung = rng.below(HUNG) == 0;
                    let tint = TINTS[rng.below(TINTS.len() as u32) as usize];
                    let phase =
                        rng.below(WEARS) as f32 * core::f32::consts::TAU / f32::from(WEARS as u8);
                    let wear = rng.below(WEARS) as f32 / f32::from(WEARS as u8 - 1);

                    if !hung {
                        continue;
                    }
                    items.push(Tube {
                        at: Vec3::new(
                            (x as f32 + 0.5) * CELL,
                            (y as f32 + 0.5) * CELL,
                            level as f32 * LEVEL + CEILING - DROP,
                        ),
                        tint,
                        phase,
                        wear,
                    });
                }
            }
        }

        Self { items, time: 0.0 }
    }

    /// Fait battre les tubes du temps écoulé.
    pub fn advance(&mut self, dt: f32) {
        self.time += dt;
    }

    /// Les tubes qui éclairent cette image, les plus proches de l'œil d'abord.
    ///
    /// **Allumés d'abord, triés ensuite** : un tube éteint n'entre pas dans le lot, donc il
    /// ne prend pas la place d'un tube allumé plus loin. Trier avant éteindre donnerait huit
    /// voisins dont la moitié ne rend rien.
    ///
    /// **Le carré de la distance suffit à ordonner**, et évite une racine par tube : la
    /// comparaison ne change pas, et il y en a plusieurs centaines par image.
    fn live(&self, eye: Vec3) -> Vec<Light> {
        let mut lit: Vec<(f32, Light)> = self
            .items
            .iter()
            .filter_map(|tube| {
                let level = beat(self.time, tube.phase, tube.wear);
                (level > 0.0).then(|| {
                    let span = tube.at - eye;
                    let shade = |channel: u8| (f32::from(channel) * level) as u8;
                    (
                        span.dot(span),
                        Light {
                            position: tube.at,
                            radius: REACH,
                            color: Color::new(
                                shade(tube.tint[0]),
                                shade(tube.tint[1]),
                                shade(tube.tint[2]),
                                0xFF,
                            ),
                        },
                    )
                })
            })
            .collect();

        lit.sort_by(|here, there| here.0.total_cmp(&there.0));
        lit.truncate(MAX_LIGHTS);
        lit.into_iter().map(|(_, light)| light).collect()
    }

    /// Pose les tubes de cette image sur le contexte.
    ///
    /// **Avant toute soumission, et c'est une contrainte et non un ordre de goût** : une
    /// lumière posée après n'éclaire pas le lot déjà passé.
    pub fn submit(&self, context: &mut Context, eye: Vec3) -> Result<(), CoreError> {
        context.set_lights(&self.live(eye))
    }
}

/// L'intensité d'un tube à cet instant, de zéro à un.
///
/// **Un tube fatigué ne clignote pas régulièrement.** Il tient allumé, puis lâche quelques
/// fois de suite, puis se rétablit — et c'est l'irrégularité qui fait le registre. Une
/// alternance périodique se lirait comme un gyrophare.
///
/// **Une somme de trois battements de périodes incommensurables, seuillée** : elle ne se
/// répète jamais à l'identique sur la durée d'une partie, sans qu'aucun générateur n'entre
/// ici. La phase décale la même forme d'un tube à l'autre, ce qui coûte une addition là où
/// des formes distinctes coûteraient autant de réglages à accorder entre eux.
///
/// **C'est le seuil que l'usure déplace, pas l'amplitude.** Un tube n'éclaire pas moins en
/// vieillissant, il s'éteint plus souvent : à usure nulle le seuil est sous le minimum de la
/// somme et le tube ne bronche qu'à peine, à usure pleine il ne repasse au-dessus que par
/// saccades.
///
/// **Les sept cotes viennent de l'exemple d'éclairage du moteur**, qui vise le même
/// registre. Elles se jugent ensemble et à l'écran.
fn beat(time: f32, phase: f32, wear: f32) -> f32 {
    let t = time + phase;
    let wave = |period: f32| (t * core::f32::consts::TAU / period).sin();
    let mix = wave(0.37) * 0.5 + wave(1.13) * 0.3 + wave(2.9) * 0.2;

    // Le seuil coupe franchement : un tube s'éteint, il ne s'estompe pas.
    if mix < -0.95 + wear * 1.45 {
        return 0.0;
    }
    // Au-dessus, une variation qui reste haute — le tube vacille sans jamais retrouver
    // tout à fait sa pleine lumière.
    0.72 + mix * 0.28
}
