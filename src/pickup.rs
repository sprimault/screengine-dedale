// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les munitions et les fioles posées dans le labyrinthe.
//!
//! **Le décor ne les connaît pas**, et c'est la frontière que ce module exerce : un
//! objet posé n'a pas de portail, donc pas d'adjacence, donc rien de ce que le balayage
//! traverse ne s'applique à lui. C'est le jeu qui le place et qui le testera contre la
//! pose du joueur — exactement comme pour le volume d'une créature.
//!
//! **Ils sont de la traversée et non de la course** : une carte rechargée les rend tous,
//! là où ce qu'on a déjà pris vit dans [`Run`](crate::game::Run). Le symptôme à guetter
//! serait un compte de munitions rangé ici.
//!
//! **Une seule vue par sorte, et un quadrilatère orienté caméra** : un objet posé se voit
//! pareil de tous les côtés, donc rien à choisir selon l'angle et aucune grille de
//! planche à décrire — c'est le chemin de l'arme et non celui des créatures.
//!
//! **Et aucune tache d'ombre sous eux**, à l'inverse des créatures : la tache existe
//! là-bas parce qu'une silhouette haute posée sur un sprite axial paraît flotter. Un
//! objet de cinquante centimètres que le balayage pose au sol touche le sol dans l'image,
//! et une tache de sa taille ne se verrait pas. Si à l'écran ils flottent malgré tout,
//! [`blot`](crate::blot::blot) est déjà là pour une.

use std::sync::Arc;

use screengine_play::{
    Affine3, Angle, Color, Context, CoreError, Error, Sprite, SpriteOrientation, Texture, Vec3,
    World, load_png_masked,
};

use crate::body::Body;
use crate::maze::grid::Grid;

#[cfg(test)]
mod tests;

/// La vignette d'une boîte de munitions.
const BOX_SHEET: &[u8] = include_bytes!("../assets/sprites/munitions.png");

/// Celle d'une fiole de soin.
const VIAL_SHEET: &[u8] = include_bytes!("../assets/sprites/fiole.png");

/// Le côté d'une vignette, en texels.
///
/// **Les coordonnées de texture du moteur sont en texels et non normalisées**, et c'est
/// le piège qui a coûté cette découverte : écrites de zéro à un, elles ne prennent qu'un
/// **seul texel** — celui du coin, transparent dans les deux vignettes puisque l'objet
/// est détouré au centre. L'objet disparaît alors sans qu'aucune erreur soit rendue, et
/// le relevé de refus ne voit rien non plus.
///
/// `MAX_TEXEL_COORD` nommait pourtant l'unité, et `weapon.rs` passe déjà cette même cote
/// pour sa planche. Ce qui a trompé est l'habitude des coordonnées normalisées.
const SIDE: f32 = 512.0;

/// Le gabarit d'un objet posé, en unités de monde.
///
/// **Un quart d'unité de demi-étendue, soit cinquante centimètres de côté** : une case
/// fait trois unités de large, donc un objet occupe le sixième d'un couloir. Plus petit,
/// il disparaît dans le grain d'une résolution interne basse ; plus gros, une boîte de
/// munitions a la taille d'une caisse.
///
/// **Cubique, et c'est ce qui le pose juste** : la demi-hauteur sert de cote de pose —
/// le balayage descend le volume jusqu'au sol et son centre s'arrête à cette hauteur —,
/// donc une étendue verticale différente ferait un objet enterré ou flottant.
const HALF: Vec3 = Vec3::new(0.25, 0.25, 0.25);

/// Le motif qui alterne les deux sortes sur les cases semées.
///
/// **Un motif plutôt qu'une coupure**, et c'est le semis qui l'impose : il répartit ses
/// cases **par étage**, donc les premiers rangs sont tous en haut. Poser les munitions
/// sur les premiers rangs et les fioles sur les derniers les aurait séparées par étage,
/// ce qui n'est pas une intention mais un effet de bord.
///
/// **Trois pour deux, répété** : c'est la proportion de six boîtes pour quatre fioles,
/// réduite, et c'est elle qui décide de [`SPOTS`] — un compte qui n'est pas un multiple
/// de cinq rendrait les deux comptes approximatifs.
const PATTERN: [Kind; 5] = [Kind::Box, Kind::Vial, Kind::Box, Kind::Vial, Kind::Box];

/// Combien d'objets un labyrinthe porte, les deux sortes confondues.
///
/// **Vingt, soit douze boîtes et huit fioles** sur cinq cent douze cases : un objet toutes
/// les vingt-cinq cases, là où une créature tombe toutes les trente-deux.
///
/// **Dix d'abord, et c'était trop peu, mesuré à l'écran** : une traversée entière sans en
/// croiser un seul. Un objet pour cinquante et une cases suppose de regarder le sol là où
/// l'on regarde devant, et quarante-quatre centimètres se perdent dans un couloir sombre.
///
/// **Ce n'est pas la rareté des munitions pour autant**, et les deux ne se confondent
/// pas : ce qu'une boîte **rend** est le levier de la rareté, et il se règle avec le
/// ramassage. Des boîtes qu'on trouve mais qui donnent peu laissent la dépense d'un tir
/// sur une paroi se juger, ce que l'étape 14 demandera ; des boîtes introuvables ne font
/// que cacher le mécanisme.
///
/// **Un multiple de cinq**, ce que [`PATTERN`] impose : sinon le dernier motif est
/// tronqué et la proportion annoncée devient fausse.
pub const SPOTS: usize = 20;

// Le compte doit être un multiple du motif, sans quoi le dernier est tronqué et la
// proportion annoncée par `SPOTS` devient fausse — six et quatre deviendraient sept et
// trois sur un compte de dix augmenté d'une unité. À la compilation, parce que c'est une
// relation entre deux constantes et que rien à l'exécution ne la rattraperait.
const _: () = assert!(SPOTS % PATTERN.len() == 0);

/// Ce qu'un objet posé donnera.
///
/// **Ce qu'il donne n'est pas ici**, et c'est volontaire : ce module le pose et le
/// montre, le ramassage est de `E6.3`. La sorte suffit à choisir sa vignette.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Une boîte de munitions.
    Box,
    /// Une fiole de soin.
    Vial,
}

/// Un objet posé : sa sorte, sa case, et le centre de son volume.
///
/// **La pose est gardée et non recalculée** : le balayage l'a résolue une fois, et la
/// refaire à chaque image pour retrouver ce qu'on sait déjà serait un balayage par objet
/// et par image.
///
/// **Et la case est gardée aussi, pour le plan de contrôle** — qui dessine en cases et
/// non en unités de monde. La déduire de la pose demanderait de diviser par la cote d'une
/// case, soit de recopier ici ce que l'export sait : une conversion de plus à tenir juste
/// le jour où une cote bouge.
pub struct Pickup {
    /// Ce qu'il est.
    pub kind: Kind,
    /// La case qui le porte, pour le plan de contrôle.
    pub spot: (u32, u32, u32),
    /// Le centre de son volume, dans le monde.
    pub at: Vec3,
}

/// Les objets d'un labyrinthe, et les deux vignettes qu'ils partagent.
///
/// **Les vignettes sont décodées une fois et portées ici**, non par chaque objet : dix
/// objets qui décoderaient leur planche en liraient dix pour deux images distinctes.
/// C'est la mesure qui a fait partager les planches des créatures — les épreuves y
/// passaient de 1,8 à 9 secondes.
pub struct Pickups {
    /// La vignette des boîtes.
    boxes: Arc<Texture>,
    /// Celle des fioles.
    vials: Arc<Texture>,
    /// Ce qui est posé.
    items: Vec<Pickup>,
}

impl Pickups {
    /// Pose un objet sur chaque case semée, en alternant les deux sortes.
    ///
    /// **La pose passe par [`Body::stand`]**, qui descend le volume par un balayage
    /// jusqu'au sol : c'est la seule façon de poser juste sur un décor dont le sol est
    /// tantôt plat, tantôt en marches, tantôt en rampe, sans avoir à savoir lequel.
    pub fn scatter(spots: &[(u32, u32, u32)], grid: &Grid, map: &World) -> Result<Self, Error> {
        let items = spots
            .iter()
            .enumerate()
            .map(|(rank, &spot)| Pickup {
                kind: PATTERN[rank % PATTERN.len()],
                spot,
                at: Body::stand(HALF, grid, map, spot).centre(),
            })
            .collect();

        Ok(Self {
            boxes: Arc::new(load_png_masked(BOX_SHEET)?),
            vials: Arc::new(load_png_masked(VIAL_SHEET)?),
            items,
        })
    }

    /// Ce qui est posé, pour le plan de contrôle.
    pub fn items(&self) -> &[Pickup] {
        &self.items
    }
}

/// Soumet les objets posés, une planche par lot.
///
/// **Deux lots et non un**, et ce n'est pas un choix : la texture d'une soumission vaut
/// pour le lot entier, donc deux vignettes font deux appels. Les grouper demanderait une
/// planche commune, soit une grille à décrire pour deux images qui n'en ont pas besoin.
///
/// **Orientés caméra et non axiaux** : un objet posé n'a pas de face, donc rien à choisir
/// selon l'angle sous lequel on le regarde. C'est aussi ce qui le dispense de normale —
/// un quadrilatère orienté caméra n'en porte pas, et il garde l'atténuation par la
/// distance seule.
///
/// Comme la scène, les créatures et l'arme, cette fonction ne lit ni horloge, ni entrée,
/// ni tampon de sortie : le chemin de rendu hors fenêtre de l'étape 8 l'appellera telle
/// quelle.
pub fn submit(context: &mut Context, pickups: &Pickups) -> Result<(), CoreError> {
    for (kind, texture) in [(Kind::Box, &pickups.boxes), (Kind::Vial, &pickups.vials)] {
        let sprites: Vec<Sprite> = pickups
            .items
            .iter()
            .filter(|item| item.kind == kind)
            .map(|item| Sprite {
                center: item.at,
                half_width: HALF.x,
                half_height: HALF.z,
                u0: 0.0,
                v0: 0.0,
                u1: SIDE,
                v1: SIDE,
                // Un objet posé ne roule pas : le roulis sert une lueur ou une
                // étincelle, et il tournerait ici la vignette dans son plan.
                roll: Angle::from_radians(0.0),
                color: Color::new(0xFF, 0xFF, 0xFF, 0xFF),
            })
            .collect();

        context.submit_sprites(
            Affine3::IDENTITY,
            &sprites,
            Some(texture),
            SpriteOrientation::Facing,
        )?;
    }

    Ok(())
}
