// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! L'arme en main : un sprite du monde posé devant l'œil.
//!
//! **Ni un quadrilatère plein cadre, ni un dessin sur le tampon fini.** C'est un
//! objet du monde, orienté caméra, soumis comme un autre : il reçoit donc le
//! brouillard, les lumières et la courbe de sortie comme le reste du décor, là où
//! des mains composées sur l'image finie resteraient à pleine lumière dans un
//! couloir sombre. Le moteur n'y gagne aucune notion de jeu : il voit un
//! quadrilatère.
//!
//! **Elle est un état de partie**, jetée au rechargement de la carte. Sa phase de
//! balancement avance avec la distance **réellement parcourue**, pas avec le
//! temps.
//!
//! **Et c'est ce qui en fait un instrument de mesure.** Le décor filtre le
//! déplacement, donc le balancement s'arrête de lui-même contre un mur sans qu'une
//! ligne d'ici le sache : des mains qui continueraient dénonceraient un déplacement
//! appliqué avant la collision.

use std::sync::Arc;

use screengine_play::{
    Affine3, Angle, Camera, Color, Context, Error, Sprite, SpriteOrientation, Texture, Vec3,
    load_png_masked,
};

use crate::player::HALF;

#[cfg(test)]
mod tests;

/// La planche des mains, au repos.
///
/// **Chargée masquée et non pas simplement lue** : sans le format à masque
/// l'alpha est ignoré, le quadrilatère entier se dessine, et les texels que le
/// détourage avait vidés reparaissent avec leur couleur — le fond du canon en
/// tête.
const REST: &[u8] = include_bytes!("../assets/sprites/mains-revolver-repos.png");

/// Celle du coup de feu, montrée le temps d'un éclair.
const FIRE: &[u8] = include_bytes!("../assets/sprites/mains-revolver-tir.png");

/// Combien de temps la pose de tir reste à l'écran, en secondes.
///
/// **Deux ou trois images, pas plus** : un éclair de bouche est bref, et le tenir
/// plus longtemps donne une arme qui reste figée en avant au lieu de claquer. La
/// durée se compte en secondes et non en images, pour ne pas dépendre de la
/// cadence.
const FLASH: f32 = 0.05;

/// Le côté de la planche, en texels.
const SIDE: f32 = 512.0;

/// La distance de l'œil au plan de l'arme, en unités de monde.
///
/// **Elle est bornée par le corps, et c'est un invariant** : un mur ne peut
/// couper l'arme que si l'œil s'en approche à moins que cette distance, ce que la
/// demi-largeur du corps interdit. Le contrôle est à la compilation, plus bas.
const DISTANCE: f32 = 0.25;

/// Ses demi-étendues, **en unités de monde** et jamais en pixels.
///
/// En pixels, l'arme changerait de taille avec la résolution interne, qui est un
/// réglage. Neuf centièmes à vingt-cinq de distance sous-tendent vingt degrés de
/// demi-angle, soit environ le tiers de la largeur de l'image.
const EXTENT: (f32, f32) = (0.09, 0.09);

/// Son décalage vers la droite et vers le bas, depuis l'axe du regard.
///
/// **Assez bas pour que le quadrilatère sorte de l'image par le bas** : centré, il
/// donne des mains qui flottent au milieu de l'écran et paraissent lointaines. Ce
/// qui les met devant l'œil, c'est qu'on n'en voie pas le bas.
///
/// **Le latéral n'est plus un cadrage mais un alignement**, et c'est le réticule qui
/// l'a imposé : il marque le centre de la vue, d'où le rayon part, et une arme posée
/// ailleurs désigne un autre point que celui qu'on touche — on tire à côté de ce qu'on
/// pointe. La bouche du canon est dessinée à deux millièmes à gauche du centre de sa
/// planche, donc le quadrilatère se décale d'autant à droite pour l'amener dans l'axe.
/// `le_canon_tombe_dans_l_axe_du_regard` tient les deux ensemble.
const OFFSET: (f32, f32) = (0.0026, -0.094);

/// Le débattement du balancement, latéral puis vertical.
const SWAY: (f32, f32) = (0.016, 0.010);

/// La distance parcourue pour un pas complet, en unités de monde.
const STRIDE: f32 = 1.7;

/// Le rappel de l'inertie du lacet, par pas de temps.
///
/// Trop bas, l'arme colle au centre et ne réagit pas au regard ; trop haut, elle
/// part et ne revient jamais.
const RECOIL: f32 = 0.25;

/// L'inclinaison que le pas donne à l'arme, en radians.
///
/// **Le roulis est ce qu'aucun déplacement du centre ne rend** : l'arme penche en
/// marchant, et c'est cette inclinaison qui fait lire un poids dans la main plutôt
/// qu'une image collée devant l'œil. Deux degrés suffisent — au-delà, elle
/// tangue.
const ROLL_STRIDE: f32 = 0.035;

/// Celle que le virage y ajoute, par radian de retard du lacet.
///
/// Elle penche du côté vers lequel on tourne, et elle domine la précédente dès
/// qu'on pivote : un virage se voit plus qu'un pas.
const ROLL_DRAG: f32 = 0.8;

// **L'arme tient derrière le corps**, et c'est ce qui empêche un mur de la
// couper : il faudrait que l'œil approche la paroi de moins que cette distance,
// ce que la demi-largeur du corps rend impossible. Relation entre constantes,
// donc vérifiée ici et non dans une épreuve.
const _: () = assert!(DISTANCE < HALF.x);

/// L'arme que le joueur tient : ses planches, et où elle en est de son balancement.
pub struct Weapon {
    /// La planche de repos, chargée une fois.
    rest: Arc<Texture>,
    /// Celle du coup de feu.
    fire: Arc<Texture>,
    /// Ce qui reste de l'éclair, en secondes, ou zéro au repos.
    flash: f32,
    /// La phase du pas, en tours — sa partie fractionnaire seule compte.
    stride: f32,
    /// Le retard du lacet sur la caméra, qui décale l'arme quand on tourne.
    drag: f32,
    /// Le lacet au pas précédent.
    last_yaw: f32,
}

impl Weapon {
    /// Charge la planche et pose l'arme au repos.
    ///
    /// # Erreurs
    ///
    /// Jamais pour une planche du dépôt : elle est intégrée au binaire et le
    /// chargement n'échoue que sur un fichier illisible. L'erreur remonte quand
    /// même, parce qu'un `expect` sur un chemin atteignable n'a pas sa place et
    /// que l'appelant sait déjà en rendre compte.
    pub fn new(yaw: f32) -> Result<Self, Error> {
        Ok(Self {
            rest: Arc::new(load_png_masked(REST)?),
            fire: Arc::new(load_png_masked(FIRE)?),
            flash: 0.0,
            stride: 0.0,
            drag: 0.0,
            last_yaw: yaw,
        })
    }

    /// Montre la pose de tir, le temps d'un éclair.
    ///
    /// **Elle ne fait que cela**, et c'est le périmètre : pas de rayon, pas de
    /// point d'impact, pas de test contre une créature. Le tir se résout en deux
    /// temps à l'étape 4 — le rayon contre le décor est l'affaire du moteur, le
    /// test contre un monstre celle du jeu —, et le geste sera déjà là pour y
    /// accrocher ses conséquences.
    ///
    /// Un appui pendant l'éclair le relance depuis sa durée pleine, plutôt que de
    /// s'ajouter : une arme ne tire pas deux fois en deux images.
    pub fn shoot(&mut self) {
        self.flash = FLASH;
    }

    /// Avance le balancement de ce qui a été parcouru, et suit le lacet.
    ///
    /// **La distance arrive mesurée, elle ne se déduit pas des touches** : le décor
    /// freine le déplacement, et c'est ce qui a vraiment été parcouru qui fait
    /// marcher.
    pub fn advance(&mut self, travel: f32, yaw: f32, dt: f32) {
        self.stride += travel / STRIDE;
        self.drag += (yaw - self.last_yaw - self.drag) * RECOIL;
        self.last_yaw = yaw;

        // L'éclair s'éteint au temps et non à la distance : il ne dépend pas de la
        // marche, et il ne se prolonge pas quand on s'arrête.
        self.flash = (self.flash - dt).max(0.0);
    }
}

/// Soumet l'arme, **après le décor**.
///
/// **Le seul ordre qui vaille** : elle est la plus proche de l'œil, donc le tampon
/// de profondeur la laisserait gagner de toute façon, mais la soumettre en dernier
/// évite qu'un décor très proche la rejette à égalité.
///
/// Comme la scène, cette fonction ne lit ni horloge, ni entrée, ni tampon de
/// sortie : le chemin de rendu hors fenêtre de l'étape 8 l'appellera telle quelle.
pub fn submit(
    context: &mut Context,
    weapon: &Weapon,
    camera: &Camera,
) -> Result<(), screengine_play::screengine::Error> {
    let pose = Affine3::from_rotation_translation(camera.orientation, Vec3::ZERO);
    let ahead = pose.transform_vector(Vec3::new(1.0, 0.0, 0.0));
    // Le repère de la caméra neutre : elle regarde le +X, sa droite est le −Y.
    let right = pose.transform_vector(Vec3::new(0.0, -1.0, 0.0));
    let up = pose.transform_vector(Vec3::new(0.0, 0.0, 1.0));

    // **La figure de Lissajous du pas, de rapport deux** : le latéral à la
    // fréquence du pas, le vertical au double — un pas gauche et un pas droit
    // descendent tous les deux. C'est ce rapport, et non l'amplitude, qui fait
    // lire une marche plutôt qu'un flottement.
    let phase = weapon.stride * core::f32::consts::TAU;
    let swing = phase.sin() * SWAY.0 - weapon.drag * 0.5;
    let bob = (phase * 2.0).cos() * SWAY.1;

    let center =
        camera.position + ahead * DISTANCE + right * (OFFSET.0 + swing) + up * (OFFSET.1 + bob);

    // Le roulis porte l'inclinaison, et il a deux sources : le pas, qui fait
    // pencher l'arme à chaque enjambée, et le virage, qui l'incline du côté vers
    // lequel on tourne. Aucun déplacement du centre ne rend ni l'un ni l'autre.
    let roll = Angle::from_radians(weapon.drag * ROLL_DRAG + phase.sin() * ROLL_STRIDE);

    context.submit_sprites(
        Affine3::IDENTITY,
        &[Sprite {
            center,
            half_width: EXTENT.0,
            half_height: EXTENT.1,
            u0: 0.0,
            v0: 0.0,
            u1: SIDE,
            v1: SIDE,
            roll,
            color: Color::new(0xFF, 0xFF, 0xFF, 0xFF),
        }],
        Some(if weapon.flash > 0.0 {
            &weapon.fire
        } else {
            &weapon.rest
        }),
        SpriteOrientation::Facing,
    )
}
