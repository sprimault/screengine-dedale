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
    Affine3, Camera, Color, Context, CoreError, Error, Lightmaps, Texture, Visibility, World,
    load_png,
};

use crate::maze::export;
use crate::maze::grid::{Grid, Settings};
use crate::tube::Tubes;

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

/// De combien le sur-éclairement multiplie ce qu'une lightmap et une texture donnent.
///
/// **Zéro, et c'est l'inverse de ce qu'on croit devoir faire sur un décor sombre.** Essayé
/// à un pour remonter un décor jugé trop noir : c'est la faute que l'exemple d'éclairage du
/// moteur nomme et explique — « à un, le sur-éclairement double une lightmap qui atteint
/// déjà l'unité sous une trouée : le couloir se lave, la mousse vire au vert vif et les
/// flaques d'ombre disparaissent ».
///
/// **Ce qui fait le registre est le contraste entre les flaques et le noir, jamais la
/// quantité de lumière.** Ce qui manquait n'était donc pas de la lumière mais un **plancher
/// de noir**, et il se pose par la courbe de sortie — voir [`GRADE`].
const OVERBRIGHT: u32 = 0;

/// La courbe de sortie : son gamma, ses trois gains, ses trois décalages.
///
/// **Les décalages relèvent le noir, et c'est la pièce qui manquait.** Le calcul de
/// lightmaps du moteur part de zéro là où rien ne porte — il n'a pas de terme ambiant, et
/// le contrat n'en expose aucun —, donc une surface hors de portée d'une lampe est
/// **exactement** noire. Deux centièmes ajoutés à la sortie suffisent à ce qu'on devine
/// encore les murs hors des flaques, sans laver quoi que ce soit.
///
/// **Le gamma assombrit au lieu d'éclaircir**, sous un : il creuse les demi-teintes et
/// laisse les flaques, ce qui est le registre visé. Monter le gamma remonterait le décor
/// entier, flaques comprises, et rendrait la scène plate — le défaut qu'on vient de fuir.
///
/// **Les gains refroidissent d'un souffle**, le bleu au-dessus du rouge : c'est ce que
/// `ROADMAP.md` appelle des sources rares et froides, et ce qui distingue un souterrain
/// d'une cave éclairée à la bougie.
///
/// **Reprise de l'exemple d'éclairage du moteur**, qui vise le même registre sur un décor
/// aussi sombre. Les sept valeurs se jugent à l'écran, ensemble : elles composent une seule
/// table par canal, et bouger le gamma seul déplace ce que les gains voulaient dire.
const GRADE: (f32, [f32; 3], [f32; 3]) = (0.92, [0.92, 0.98, 1.06], [0.012, 0.014, 0.022]);

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
    /// Les tubes au plafond, et leur battement.
    pub tubes: Tubes,
    /// Les lightmaps cuites, un atlas par cellule.
    ///
    /// **Du monde et non de la partie**, et c'est ce qui décide de leur place : elles sont
    /// un cache de la carte, donc elles se refont avec elle et survivent à une course
    /// perdue. Les ranger dans la traversée les ferait recuire à chaque relance — une
    /// seconde de calcul pour un décor qui n'a pas bougé.
    lightmaps: Lightmaps,
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

        // **Toutes les cellules sont cuites ici, et non celles que la caméra voit** : une
        // lightmap est un cache de la carte et non de la vue, donc la cuire en chemin
        // ferait allouer un atlas au milieu d'une image — ce que le moteur refuse, et ce
        // qui se verrait de toute façon comme un décor qui s'allume par morceaux.
        //
        // **Les lumières, elles, viennent de l'export** : une par cellule de case, une par
        // cage. C'est la géométrie qui décide de ce qu'une lampe atteint, donc les écrire
        // plus tard obligerait à réexporter la carte.
        let mut lightmaps = Lightmaps::new(&map)?;
        for index in 0..map.cell_count() {
            if let Some(id) = map.cell_id(index) {
                lightmaps.build(&map, id)?;
            }
        }

        Ok(Self {
            tubes: Tubes::hang(&maze),
            maze,
            map,
            materials,
            lightmaps,
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
/// Pose sur le contexte tous les réglages que cette image demande.
///
/// **Extraite à la troisième fois qu'un réglage ajouté ici a cassé un oracle.** Les
/// épreuves qui comparent la traversée au décor entier doivent régler leur second contexte
/// à l'identique, sinon elles mesurent un écart de réglage et non de traversée. Recopiés,
/// ces appels ont divergé trois fois de suite — au brouillard, au sur-éclairement puis aux
/// tubes —, et chaque fois l'épreuve annonçait une divergence de traversée qui n'existait
/// pas. Un site unique est le seul remède, et c'est la règle de trois.
///
/// **Tous sont refusés une fois la soumission commencée**, donc ils se posent ensemble et
/// avant. Une lumière posée après n'éclairerait pas le lot déjà passé, et le reste rendrait
/// `InvalidState`.
///
/// **Rien ici ne lit d'horloge** : le temps des tubes est un champ du monde, avancé par le
/// pas de jeu. C'est ce qui permet au chemin hors fenêtre d'appeler cette fonction telle
/// quelle.
pub fn prepare(context: &mut Context, scenery: &Scenery, view: &View) -> Result<(), CoreError> {
    context.set_fog(FOG, FOG_START, FOG_END)?;
    context.set_overbright(OVERBRIGHT)?;
    context.set_grade(GRADE.0, GRADE.1, GRADE.2)?;
    // Les tubes prennent l'œil de la caméra : le moteur n'en accepte que huit par image, et
    // ce sont les plus proches qui rendent quelque chose.
    scenery.tubes.submit(context, view.camera.position)?;
    context.set_camera(view.camera)
}

pub fn submit(
    context: &mut Context,
    scenery: &Scenery,
    view: &View,
) -> Result<Visibility, CoreError> {
    // **Le brouillard se pose ici et à chaque image**, avec la caméra : c'est un
    // réglage d'image, refusé une fois la soumission commencée, et le poser une seule
    // fois obligerait à savoir laquelle est la première — ce que cette fonction, qui ne
    // garde rien, ne peut pas savoir. L'appel est une écriture de quatre champs.
    prepare(context, scenery, view)?;
    context.submit_world_visible(
        Affine3::IDENTITY,
        &scenery.map,
        view.cell,
        // **Les lightmaps cuites au chargement**, et c'est ce qui fait passer la
        // soumission du chemin non éclairé au chemin éclairé. Sans elles, les
        // lumières que l'export écrit n'atteignent rien : elles sont dans le
        // fichier depuis l'étape 1, et c'est la cuisson qui les fait exister.
        Some(&scenery.lightmaps),
        |rank| scenery.materials.get(rank as usize),
    )
}
