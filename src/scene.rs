// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! L'état du monde, et la scène qu'on en soumet au moteur.
//!
//! **`submit` ne lit ni horloge, ni entrée, ni tampon de sortie**, et c'est sa
//! raison d'être : la boucle de `screengine-play` ouvre une fenêtre, donc elle ne
//! servira pas à l'image animée du `README`, qui appellera le moteur directement.
//! Les deux chemins doivent construire la même scène, ce qui n'est possible que
//! si la construction vit hors du rappel. Écrite dedans, elle serait à défaire le
//! jour venu.
//!
//! Elle ne mute rien non plus : une scène se soumet deux fois sur la même pose
//! sans que rien ne bouge entre les deux, ce qu'un rendu en plusieurs passes fait
//! par construction.

use std::sync::Arc;

use screengine_play::{
    Affine3, Camera, Color, Context, CoreError, Error, Texture, Visibility, World, load_png,
};

use crate::maze::export;
use crate::maze::grid::{Grid, Settings};

#[cfg(test)]
mod tests;

/// La planche des murs.
///
/// **La variante moussue et non la pierre propre, et c'est le brouillard qui l'a
/// décidé** : mesurées, les deux planches claires valaient 115 et 148 de clarté
/// moyenne sur 255, là où les moussues valent 34 et 53. Une teinte presque noire
/// doit éteindre beaucoup pour se voir sur un décor clair, donc elle salit le
/// proche avant de creuser le lointain ; contre un décor sombre, la même courbe
/// se lit sans forcer.
///
/// **Les deux planches claires restent dans `assets/`**, et ce n'est pas un oubli :
/// l'étape 15 attache un jeu de textures au rang d'un labyrinthe dans la descente,
/// et une pierre propre y aura sa place comme thème.
const WALL: &[u8] = include_bytes!("../assets/textures/mur-mousse.png");

/// Celle du sol, du plafond et des marches, moussue pour la même raison.
///
/// **Les marches la prennent faute d'une planche à elles** : un escalier est
/// d'une seule matière, et c'est celle qu'on foule. Un matériau `stair` se
/// justifiera quand il y aura une image à lui donner.
const FLOOR: &[u8] = include_bytes!("../assets/textures/sol-pave-mousse.png");

/// La teinte du brouillard, vers laquelle tout s'éteint avec la distance.
///
/// **Presque noire, et bleutée d'un rien** : le registre du jeu est celui d'un
/// souterrain, donc ce qui se perd au loin se perd dans le noir et non dans une brume
/// claire. Le bleu léger est ce qui distingue cette obscurité d'un pixel éteint — sans
/// lui, le fond et le brouillard seraient le même noir, et on ne saurait pas en
/// regardant si le lointain est fermé ou simplement vide.
///
/// Elle se juge à l'écran, comme toute cote d'ambiance.
///
/// **Les trois cotes du brouillard attendent l'éclairage cuit, et ne se règlent pas
/// avant.** Elles ont d'abord été prises contre des planches claires, puis revues contre
/// les moussues — trois fois plus sombres —, et le décor reste pourtant clair : rien n'est
/// cuit, donc aucune cote ici ne se juge contre ce que le jeu montrera. C'est la raison de
/// l'ordre suivi : les planches d'abord, parce qu'une lightmap cuite sur des planches
/// claires serait à recuire ; le brouillard en dernier, quand il a enfin le décor contre
/// lequel il a été pensé.
pub const FOG: Color = Color::new(0x0A, 0x0C, 0x14, 0xFF);

/// Où le brouillard commence à mordre, en unités de monde depuis l'œil.
///
/// **Une case et demie**, une case valant quatre unités — trois de cellule et une de
/// passage. En deçà, rien ne s'éteint : le mur d'en face d'un couloir doit garder sa
/// matière, sinon c'est le décor proche qui perd sa lisibilité au lieu que le lointain
/// gagne sa profondeur.
pub const FOG_START: f32 = 6.0;

/// Où il est total, en unités de monde depuis l'œil.
///
/// **Cinq cases et demie**, ce que `docs/conception.md` appelle fermer la vue au bout de
/// quelques cellules. Le dégagement du semis est à six cases, choisi pour rester
/// au-delà de ce que le brouillard laisse voir — les deux cotes se lisent ensemble, et
/// celle-ci ne doit pas le rattraper.
///
/// **Au-delà, un mur vaut exactement le fond**, et le relevé de visibilité le compte
/// alors comme un point non peint. C'est voulu — il n'y a plus rien à voir —, mais une
/// enfilade plus longue que cette cote fait baisser son pourcentage sans que l'image
/// soit amputée. Si le relevé se met à crier, c'est le premier endroit à regarder.
pub const FOG_END: f32 = 22.0;

/// Ce qu'un pixel porte quand aucun triangle ne l'a peint.
///
/// **Ce n'est pas un réglage mais une conséquence**, et le contrat du moteur la
/// nomme : un pixel qu'aucun triangle n'a peint est infiniment lointain, donc il
/// prend la couleur du brouillard de lui-même. Elle se **dérive** donc de la teinte
/// ci-dessus, et ne s'écrit pas à côté : les deux valeurs doivent être la même, et
/// deux constantes jumelles divergent au premier réglage.
///
/// **Publiée pour le relevé**, qui compte les points peints d'une image et a besoin
/// de savoir à quoi ressemble le vide. Un relevé qui supposerait le noir serait devenu
/// muet le jour où ce brouillard est arrivé, sans rien dire.
pub const BACKGROUND: [u8; 3] = [FOG.r, FOG.g, FOG.b];

/// L'état du monde : la carte chargée, et ce qu'il faut pour la dessiner.
///
/// **Rien de la partie n'entre ici** — ni vie, ni score, ni pose de caméra. Le
/// monde est rechargeable à chaud, la partie est jetée au rechargement, et c'est
/// cette séparation qui rend l'édition possible.
pub struct Scenery {
    /// Le labyrinthe engendré, que le plan de contrôle relit.
    pub maze: Grid,
    /// La carte que le moteur en a tirée.
    pub map: World,
    /// Une texture par emplacement de matériau, dans l'ordre de la carte.
    materials: Vec<Arc<Texture>>,
}

impl Scenery {
    /// Engendre un labyrinthe, l'écrit en carte, et charge de quoi l'habiller.
    pub fn new(settings: Settings) -> Result<Self, Error> {
        let maze = Grid::generate(settings);
        let map = World::load(&export::world(&maze))?;

        let wall = Arc::new(load_png(WALL).expect("planche de mur du dépôt valide"));
        let floor = Arc::new(load_png(FLOOR).expect("planche de sol du dépôt valide"));
        // Le fichier porte des noms, jamais des images : l'hôte les découvre et
        // passe ses propres handles, dans l'ordre des emplacements.
        let materials = (0..map.material_count())
            .map(|rank| match map.material_name(rank) {
                Some("wall") => Arc::clone(&wall),
                _ => Arc::clone(&floor),
            })
            .collect();

        Ok(Self {
            maze,
            map,
            materials,
        })
    }
}

/// Ce qu'une image regarde.
///
/// La cellule accompagne la pose parce que le moteur ne retient ni l'une ni
/// l'autre : il ne connaît pas le monde entre deux appels, et c'est l'hôte qui
/// suit sa caméra.
pub struct View {
    /// La pose de la caméra.
    pub camera: Camera,
    /// La cellule qui la contient, ou zéro pour « nulle part ».
    pub cell: u32,
}

/// Soumet au moteur ce que cette vue montre du monde.
///
/// Rend ce que la traversée a pu déplier : `Incomplete` dit qu'elle a atteint une
/// de ses deux bornes et que l'image s'arrête une cellule plus loin, `NoCell` que
/// la pose est hors de tout volume. Aucun des deux n'est une erreur, et un
/// labyrinthe est le premier décor qui puisse les approcher, et relever ce que
/// cette valeur dit reste à faire.
///
/// L'erreur est celle du **noyau**, et son chemin reste long là où `Context` et
/// `Visibility` sont à plat : la boucle a sa propre `Error`, et les deux ne
/// peuvent pas porter le même nom dans la même surface.
pub fn submit(
    context: &mut Context,
    scenery: &Scenery,
    view: &View,
) -> Result<Visibility, CoreError> {
    // **Le brouillard se pose ici et à chaque image**, avec la caméra : c'est un
    // réglage d'image, refusé une fois la soumission commencée, et le poser une seule
    // fois obligerait à savoir laquelle est la première — ce que cette fonction, qui ne
    // garde rien, ne peut pas savoir. L'appel est une écriture de quatre champs.
    context.set_fog(FOG, FOG_START, FOG_END)?;
    context.set_camera(view.camera)?;
    context.submit_world_visible(
        Affine3::IDENTITY,
        &scenery.map,
        view.cell,
        // Aucune lightmap : la soumission retombe alors sur le chemin non
        // éclairé, sans erreur. La carte porte pourtant ses lumières — c'est la
        // géométrie qui décide de ce qu'une lampe atteint, donc les écrire tard
        // obligerait à réexporter —, mais rien n'est cuit avant l'étape 8, et on
        // ne règle pas un tamisage dans un couloir qu'on vient d'ouvrir.
        None,
        |rank| scenery.materials.get(rank as usize),
    )
}
