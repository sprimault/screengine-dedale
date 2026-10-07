// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La marque d'un impact sur le décor : son anneau, et ce qu'on en soumet.
//!
//! **Un quadrilatère plaqué, et le contrat nomme ce cas** : il écarte quatre sommets
//! pré-orientés de sa primitive de sprite en disant qu'un hôte les soumet de toute
//! façon lui-même s'il veut une orientation libre — une affiche, un impact, une tache
//! au sol. Ce chemin existe et ne bouge pas, et la base tangente est à nous.
//!
//! **Modulée et non masquée, et ce n'est pas qu'une question d'aspect.** Le test de
//! profondeur du chemin modulé **tolère la pente** — deux pas de profondeur par pixel
//! —, ce qui fait tenir une marque dans le plan de la surface qu'elle marque, quelle
//! que soit la découpe des deux. Le chemin masqué a un test strict, et le décalage qui
//! le ferait gagner n'est publié nulle part : il demanderait une mesure. S'ajoute que
//! le registre voulu est un creux sombre, et qu'une surface modulée ne peut
//! qu'assombrir.
//!
//! **Un anneau et non une liste qui croît** : la capacité de triangles du moteur est
//! une falaise et non une dégradation — un dépassement refuse le lot entier —, donc le
//! nombre de marques vivantes se borne ici, et leur lot se soumet séparément du décor.

use std::sync::Arc;

use screengine_play::{Affine3, Color, Context, Texture, Triangle, Vec3, VertexUv};

use crate::blot::blot;

#[cfg(test)]
mod tests;

/// Le demi-côté d'une marque, en unités de monde.
///
/// **Un peu moins de deux décimètres de côté** : à la portée du tir, une marque plus
/// large couvrirait une dalle entière, et plus étroite ne ferait qu'un pixel ou deux à
/// la résolution interne. La cote se juge à l'écran, et c'est ce qu'elle attend.
const RADIUS: f32 = 0.09;

/// De combien la marque flotte au-dessus de la surface, en unités de monde.
///
/// **Un décalage de géométrie et non un biais de profondeur**, comme celui de la tache
/// d'ombre et pour une raison voisine : le test du chemin modulé tolère déjà la pente,
/// donc ce millimètre ne sert pas à gagner le test. Il sert à ne pas entrer dans la
/// surface quand le point de contact tombe exactement dans son plan et que les deux
/// découpes divergent.
///
/// **Dix fois plus petit que celui de la tache**, qui compense un quadrilatère
/// horizontal posé sur une rampe : ici le quadrilatère suit la normale de ce qu'il
/// marque, donc il n'y a aucun angle à rattraper.
const LIFT: f32 = 0.001;

/// Ce que la marque laisse passer en son centre, sur 255.
///
/// Plus sombre que la tache d'ombre : un impact est un creux, là où une ombre est un
/// appui. `255` étant le neutre, un huitième éteint franchement sans noircir.
const CORE: f32 = 0x18 as f32;

/// Le côté de la texture d'une marque, en texels.
///
/// **La moitié de celle de la tache**, et une puissance de deux que le moteur exige :
/// c'est un dégradé radial deux fois plus petit en monde, qui n'a aucun détail à
/// porter.
const SIDE: u32 = 32;

/// Combien de marques vivent en même temps.
///
/// **Seize, et c'est la lisibilité qui le borne, pas la capacité.** Le budget de
/// triangles en tiendrait des centaines ; ce qui compte est qu'un couloir ne se
/// couvre pas de taches au point qu'on ne distingue plus le dernier tir. La plus
/// ancienne s'effface quand la dix-septième arrive.
const KEEP: usize = 16;

// **Un décalage nul ferait entrer la marque dans ce qu'elle marque**, et c'est une
// relation entre constantes : elle n'a rien à faire dans une épreuve, qui ne la
// vérifierait qu'après la compilation. Son plafond vaut autant — un décalage de l'ordre
// du rayon décollerait la marque de la surface et se verrait comme une vignette posée
// devant.
const _: () = assert!(LIFT > 0.0);
const _: () = assert!(LIFT < RADIUS / 16.0);

/// Une marque posée : où, et contre quoi.
#[derive(Clone, Copy)]
struct Mark {
    /// Le point de contact, sur le plan de la surface touchée.
    at: Vec3,
    /// La normale de cette surface, unitaire.
    normal: Vec3,
}

/// Les marques vivantes, et la texture qu'elles partagent.
///
/// **Un état de partie** : il est jeté au rechargement de la carte, comme la vie ou le
/// score. Les identifiants de surface survivraient au rechargement, mais les marques
/// non — un décor remplacé n'a pas à garder les impacts du précédent.
pub struct Marks {
    /// Les marques, de la plus ancienne à la plus récente.
    ring: Vec<Mark>,
    /// Le disque modulant, engendré une fois et partagé par toutes.
    ///
    /// Sous `Arc` parce que la soumission le veut ainsi : le moteur garde la texture
    /// le temps de l'image, et le compteur est ce qui le lui permet sans copie.
    texture: Arc<Texture>,
}

impl Default for Marks {
    fn default() -> Self {
        Self::new()
    }
}

impl Marks {
    /// Un anneau vide, et son disque engendré.
    pub fn new() -> Self {
        Self {
            ring: Vec::with_capacity(KEEP),
            texture: Arc::new(blot(SIDE, CORE)),
        }
    }

    /// Pose une marque au point de contact, orientée par la normale.
    ///
    /// **Une normale nulle ne pose rien**, et c'est le seul refus : le moteur la rend
    /// nulle quand rien n'est touché, et il la rend nulle aussi sur un trajet tronqué.
    /// Sans base tangente, il n'y a pas de quadrilatère à décrire.
    pub fn add(&mut self, at: Vec3, normal: Vec3) {
        if normal.dot(normal) == 0.0 {
            return;
        }
        if self.ring.len() == KEEP {
            self.ring.remove(0);
        }
        self.ring.push(Mark { at, normal });
    }
}

/// Les deux tangentes unitaires du plan de cette normale.
///
/// **Le contrat ne les donne pas, et c'est la frontière assumée** : le moteur rend une
/// normale, l'hôte en tire les sommets. Il ne reste qu'à les prendre dans un ordre qui
/// tienne.
///
/// **L'axe de départ se choisit sur la composante la plus faible**, parce qu'un produit
/// vectoriel avec un axe presque colinéaire rend un vecteur presque nul, que
/// `normalize` ramènerait au vecteur nul — et le quadrilatère s'effondrerait sans
/// erreur. Les surfaces de ce décor étant axiales ou à quarante-cinq degrés, le cas
/// serait quotidien avec un axe fixe.
fn tangents(normal: Vec3) -> (Vec3, Vec3) {
    let (x, y, z) = (normal.x.abs(), normal.y.abs(), normal.z.abs());
    let aside = match (x <= y, x <= z, y <= z) {
        (true, true, _) => Vec3::new(1.0, 0.0, 0.0),
        (false, _, true) => Vec3::new(0.0, 1.0, 0.0),
        _ => Vec3::new(0.0, 0.0, 1.0),
    };

    let along = normal.cross(aside).normalize();
    (along, normal.cross(along).normalize())
}

/// Les quatre coins d'une marque, dans le sens qui la montre.
///
/// **Le sens de parcours décide de la face vue**, et un quadrilatère décrit à l'envers
/// est un dos de face qui disparaît — sans erreur, et c'est le défaut le plus coûteux à
/// diagnostiquer de ce moteur. L'ordre ci-dessous tourne dans le sens direct autour de
/// la normale, donc il se voit depuis le côté d'où le tir est venu.
fn corners(mark: &Mark) -> [Vec3; 4] {
    let (along, across) = tangents(mark.normal);
    let centre = mark.at + mark.normal * LIFT;

    [
        centre - along * RADIUS - across * RADIUS,
        centre + along * RADIUS - across * RADIUS,
        centre + along * RADIUS + across * RADIUS,
        centre - along * RADIUS + across * RADIUS,
    ]
}

/// Soumet les marques, **après le décor**.
///
/// **Le seul ordre qui vaille, et c'est le contrat qui le dit** : une surface modulée
/// multiplie le pixel déjà écrit dans la tuile, donc elle n'assombrit que ce qui y est
/// déjà. Soumise avant le décor, elle ne modulerait que le fond.
///
/// **Un lot par marque et non un seul lot**, et la raison n'est pas la commodité : le
/// dépassement de la capacité de triangles refuse le lot **entier**, donc seize marques
/// en un lot disparaîtraient ensemble — là où le décor, qui est soumis à part, garderait
/// les siennes. Chacune coûte deux triangles.
///
/// Comme la scène et l'arme, cette fonction ne lit ni horloge, ni entrée, ni tampon de
/// sortie : le chemin de rendu hors fenêtre de l'étape 8 l'appellera telle quelle.
pub fn submit(
    context: &mut Context,
    marks: &Marks,
) -> Result<(), screengine_play::screengine::Error> {
    /// Les coordonnées de texture d'un coin, en texels.
    const EDGE: f32 = SIDE as f32;

    let white = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
    for mark in &marks.ring {
        // La couleur est portée par le triangle et non par le sommet : c'est la
        // forme du moteur, et le blanc y laisse la modulation à la texture seule.
        let vertices: Vec<VertexUv> = corners(mark)
            .iter()
            .zip([(0.0, 0.0), (EDGE, 0.0), (EDGE, EDGE), (0.0, EDGE)])
            .map(|(&position, (u, v))| VertexUv { position, u, v })
            .collect();

        context.submit_blended(
            Affine3::IDENTITY,
            &vertices,
            &[
                Triangle {
                    indices: [0, 1, 2],
                    color: white,
                },
                Triangle {
                    indices: [0, 2, 3],
                    color: white,
                },
            ],
            Some(&marks.texture),
        )?;
    }

    Ok(())
}
