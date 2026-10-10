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

use screengine_play::{
    Affine3, Angle, Color, Context, CoreError, Sprite, SpriteOrientation, Texture, Triangle, Vec3,
    VertexUv,
};

use crate::blot::{blot, spark};

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

/// Le demi-côté d'un éclat, en unités de monde.
///
/// **Plus petit qu'une marque** : il se pose sur une silhouette d'un mètre quatre-vingt
/// et doit dire où le coup a porté, pas la recouvrir. La cote se juge à l'écran.
const SPARK_RADIUS: f32 = 0.16;

/// Combien de temps un éclat reste visible, en secondes.
///
/// **Trois images, comme l'éclair de la pose de tir et comme le recul** : les trois se
/// répondent dans le même instant, et c'est ce qui fait lire un coup plutôt qu'une
/// succession d'événements. Plus long, l'éclat deviendrait une marque posée sur la
/// créature et la suivrait mal, puisqu'il ne bouge pas avec elle.
const SPARK_TIME: f32 = 0.12;

/// Le côté de la texture d'un éclat, en texels.
const SPARK_SIDE: u32 = 32;

/// La couleur du cœur d'un éclat.
///
/// **Presque blanche, et c'est la clarté qui porte la lisibilité** : l'habillage du
/// décor changera, donc un éclat accordé à la teinte des murs d'aujourd'hui serait à
/// refaire. Un cœur clair se lit sur n'importe quel fond, et d'autant mieux que le
/// registre visé est sombre.
const SPARK_CORE: [u8; 3] = [0xFF, 0xF0, 0xC0];

/// Celle de son bord.
///
/// L'orange qui donne sa teinte au coup, là où le cœur donne sa force.
const SPARK_EDGE: [u8; 3] = [0xE0, 0x60, 0x10];

/// Une marque posée : où, et contre quoi.
#[derive(Clone, Copy)]
struct Mark {
    /// Le point de contact, sur le plan de la surface touchée.
    at: Vec3,
    /// La normale de cette surface, unitaire.
    normal: Vec3,
}

/// Un éclat vivant : où il brille, et ce qu'il lui reste à vivre.
///
/// **Il ne suit pas la créature**, et c'est assumé : il dure trois images, pendant
/// lesquelles elle parcourt au plus quelques centimètres. L'accrocher à elle demanderait
/// de la désigner, donc de tenir un identifiant pour trois images.
#[derive(Clone, Copy)]
struct Spark {
    /// Le point où le rayon est entré dans le volume.
    at: Vec3,
    /// Ce qu'il lui reste à vivre, en secondes.
    left: f32,
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
    /// Les éclats encore vivants.
    ///
    /// **Aucune borne de compte, là où les marques en ont une** : ils s'éteignent seuls
    /// en trois images, donc leur nombre est borné par la cadence de tir et non par une
    /// constante — à un coup par clic, il y en a un ou deux.
    sparks: Vec<Spark>,
    /// Le disque clair des éclats, découpé.
    flash: Arc<Texture>,
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
            sparks: Vec::new(),
            flash: Arc::new(spark(SPARK_SIDE, SPARK_CORE, SPARK_EDGE)),
        }
    }

    /// Pose un éclat là où le rayon est entré dans une créature.
    ///
    /// **C'est toute la rétroaction visuelle d'un coup porté**, avec le recul : il n'y
    /// a aucune surface de décor au point de contact, donc rien à marquer — mais il y a
    /// un point, que le départage a calculé sur le même segment que le rayon.
    pub fn flash(&mut self, at: Vec3) {
        self.sparks.push(Spark {
            at,
            left: SPARK_TIME,
        });
    }

    /// Fait vieillir les éclats, et jette ceux qui ont fini.
    ///
    /// **Les marques, elles, ne vieillissent pas** : elles restent jusqu'à ce que
    /// l'anneau les chasse. C'est la différence entre une trace laissée sur un mur et
    /// l'instant d'un coup.
    pub fn advance(&mut self, dt: f32) {
        for spark in &mut self.sparks {
            spark.left -= dt;
        }
        self.sparks.retain(|spark| spark.left > 0.0);
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
pub fn submit(context: &mut Context, marks: &Marks) -> Result<(), CoreError> {
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

    // **Les éclats en sprites et non en quadrilatères plaqués**, et c'est la nuance qui
    // décide : un impact sur une créature n'a aucune surface pour l'orienter, donc il
    // fait face à la caméra. Un quadrilatère orienté caméra ne porte pas de normale et
    // n'en prend pas — il garde l'atténuation par la distance seule, ce qui est
    // exactement ce qu'un éclat veut.
    //
    // **Un lot unique, là où les marques en ont un chacune** : leur nombre est borné par
    // la cadence de tir et non par un anneau, donc un refus pour dépassement de capacité
    // ne peut pas venir d'eux.
    let sparks: Vec<Sprite> = marks
        .sparks
        .iter()
        .map(|spark| Sprite {
            center: spark.at,
            half_width: SPARK_RADIUS,
            half_height: SPARK_RADIUS,
            u0: 0.0,
            v0: 0.0,
            u1: SPARK_SIDE as f32,
            v1: SPARK_SIDE as f32,
            roll: Angle::from_radians(0.0),
            color: white,
        })
        .collect();

    context.submit_sprites(
        Affine3::IDENTITY,
        &sparks,
        Some(&marks.flash),
        SpriteOrientation::Facing,
    )
}
