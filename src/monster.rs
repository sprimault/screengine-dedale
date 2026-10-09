// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Une créature : un corps que le décor arrête, et une planche qui la montre.
//!
//! **Le volume et le dessin sont deux choses**, et les confondre est le piège de
//! ce module : un marcheur aussi large que son dessin ne passerait nulle part. La
//! boîte qui se balaie tient dans un couloir ; la vignette qui se soumet déborde
//! d'elle, parce qu'un démon a des bras, des cornes et une ombre portée.
//!
//! **Le sprite est axial et non plein face.** Un personnage debout regardé d'en
//! haut se couche avec le plein face, ce qui est une image fausse ; en axial il
//! s'aplatit, ce qui n'est qu'une silhouette perdue. C'est l'inverse du choix de
//! l'arme, qui est plein face — donc rien ne se factorise entre les deux.
//!
//! **Il ne porte aucune normale**, et ce n'est pas un manque à combler : un
//! quadrilatère orienté caméra n'en a pas et n'en prend pas. Une créature garde
//! l'atténuation par la distance seule.
//!
//! **Les cotes du dessin appartiennent à la silhouette, pas au module**, et c'est
//! ce que la population a montré : marge de cadrage et foulée se relèvent sur
//! chaque planche, et les trois silhouettes ne s'accordent sur ni l'une ni
//! l'autre — la foulée va de seize à trente-six texels. Partagées, deux
//! silhouettes sur trois flotteraient d'un texel et patineraient du double. Elles
//! vivent donc dans [`FIGURES`], une ligne par silhouette.

use std::collections::VecDeque;
use std::sync::Arc;

use screengine_play::{
    Affine3, Angle, Camera, Color, Context, Error, Sprite, SpriteOrientation, Texture, Triangle,
    Vec3, VertexUv, World, load_png_masked,
};

use crate::blot::blot;
use crate::body::{self, Body};
use crate::maze::grid::{Grid, Side};
use crate::sheet::{self, FRAME, Motion};

#[cfg(test)]
mod tests;

/// Une silhouette : ses planches, et les cotes qu'elles imposent.
///
/// **Une entrée par silhouette plutôt qu'une table par cote.** Tout ce qui est ici
/// se relève sur la même planche, à la même occasion : deux tables indexées par le
/// même nom finiraient par ne plus s'accorder, et c'est la marge de cadrage qui l'a
/// montré — relevée d'abord pour une silhouette, elle s'est trouvée fausse pour les
/// deux autres.
///
/// **La planche de mort n'y est pas**, et le blocage est nommé : rien ne tue avant
/// l'étape 4, donc sa marge n'est pas relevée et la charger serait du poids mort.
struct Figure {
    /// Son nom, celui que portent ses fichiers de planche.
    ///
    /// **Affiché dans le titre de la fenêtre** pour la créature la plus proche :
    /// une cote qui cloche se juge à l'œil, et dire laquelle des trois cloche
    /// demande de pouvoir la nommer.
    name: &'static str,
    /// Les octets de sa planche de repos.
    idle: &'static [u8],
    /// Ceux de sa planche de marche.
    walk: &'static [u8],
    /// Ceux de sa planche de mort — seize trames qui ne se répètent pas.
    dead: &'static [u8],
    /// Ce que son repos laisse de vide sous les pieds, en texels.
    idle_margin: f32,
    /// Ce que sa marche en laisse, en texels.
    walk_margin: f32,
    /// Ce que sa mort en laisse, en texels.
    ///
    /// **Plus petite que les deux autres sur les trois silhouettes**, et c'est ce
    /// qu'on attend d'une créature qui finit au sol : il reste moins de vide sous un
    /// corps couché que sous des pieds.
    dead_margin: f32,
    /// Ce qu'un cycle de marche complet avance, en texels de planche.
    ///
    /// **En texels parce que c'est l'unité de la mesure**, et la conversion en
    /// unités de monde est l'affaire de [`Figure::stride`] : la planche se relève en
    /// texels, et un chiffre converti à la main dans une table est un chiffre qu'on
    /// ne peut plus comparer à ce qu'on remesure.
    stride: f32,
    /// Ce qu'elle vaut au compteur, abattue.
    ///
    /// **Une prime par silhouette et non un barème unique**, parce que ce qui la
    /// justifiera est déjà prévu : elles ne mourront pas de la même façon, et il
    /// faudra plus de coups pour certaines. Un barème commun serait à défaire ce
    /// jour-là, et la table est l'endroit où il s'écrit — tout ce qui sépare les
    /// trois y est déjà.
    ///
    /// **`d2` vaut le plus parce qu'elle est la plus dure à abattre**, jugé à l'écran
    /// le 2026-10-10. Ce n'est pas une cote qui le dit — les trois ont la même vie, la
    /// même vitesse et le même volume —, c'est la créature telle qu'on la vise ; et
    /// c'est bien de l'écran qu'un barème doit venir, puisque ce qu'il récompense est
    /// la difficulté ressentie.
    ///
    /// **Rien ne sépare `d1` de `d3`**, qui gardent donc l'ordre de la table, et les
    /// trois valeurs se reprendront quand leur résistance divergera. Distinctes dès
    /// maintenant pour que le compteur dise **laquelle** on a abattue, ce qu'un chiffre
    /// commun ne dirait jamais.
    bounty: u32,
}

impl Figure {
    /// Ce que la planche d'un cycle laisse de vide sous les pieds, en texels.
    ///
    /// **La marge appartient au cycle autant qu'à la silhouette**, et les six
    /// relevés ne laissent aucune case libre : `d1` passe de trois au repos à quatre
    /// en marche, `d2` de deux à trois, `d3` de trois à **deux**. Une seule valeur
    /// pour les six ferait flotter ou enterrer cinq cadrages sur six.
    ///
    /// **C'est la marge la plus fréquente du cycle qui décide, et non son
    /// minimum** — la marche de `d1` relève `[3, 4, 4, 4, 4, 4, 3, 2]`. Ancrée sur
    /// le `2`, la créature flotte pendant sept trames sur huit : la trame la plus
    /// basse est l'exception, pas la règle. Ancrée sur le `4`, cinq trames touchent
    /// exactement et les deux autres s'enfoncent d'un texel, ce qui se voit
    /// infiniment moins qu'un pied en l'air.
    fn margin(&self, motion: Motion) -> f32 {
        match motion {
            Motion::Walk => self.walk_margin,
            Motion::Dead => self.dead_margin,
            Motion::Idle => self.idle_margin,
        }
    }

    /// La distance d'un cycle de marche complet, en unités de monde.
    ///
    /// **La marche avance avec la distance et non avec le temps**, comme le
    /// balancement de l'arme : un cycle indexé sur l'horloge continuerait de défiler
    /// contre un mur, et une créature arrêtée piétinerait sur place.
    ///
    /// **Ce qu'une valeur fausse coûte se voit tout de suite** : les pieds patinent,
    /// et d'autant plus que l'écart est grand. Celle de `d1` valait `1,9` avant
    /// d'être mesurée, soit près du double de ses trente-six texels — le cycle
    /// avançait deux fois trop lentement pour la distance, et la créature glissait
    /// sur place.
    ///
    /// **Ce qu'elle coûte, et c'est assumé** : une phase indexée sur la distance
    /// avance plus vite en descendant une rampe et par à-coups dans un escalier.
    /// L'indexer sur le temps ferait patiner dès que la vitesse varie, ce qui se voit
    /// davantage et partout.
    fn stride(&self) -> f32 {
        self.stride / FRAME * (2.0 * SPRITE_HALF)
    }
}

/// Les trois silhouettes du labyrinthe, et ce que leurs planches imposent.
///
/// **Chaque cote est relevée sur sa planche, aucune n'est partagée**, et c'est le
/// relevé qui l'a imposé : les marges valent `3/4`, `2/3` et `3/2` selon la
/// silhouette et le cycle, et les foulées trente-six, seize et vingt-deux texels.
/// Une table unique aurait fait patiner deux silhouettes du double et flotter cinq
/// cadrages sur six.
///
/// **La foulée se relève sur les deux vues de profil** : l'empreinte des pieds y
/// passe de son écartement le plus serré au plus large, et le cycle porte **deux
/// pas** — ses huit trames montrent deux maxima. La foulée vaut donc deux fois cet
/// écart, et `la_foulee_annoncee_est_celle_de_la_planche` la remesure.
///
/// **Un `static` et non une constante**, pour qu'une créature puisse en garder la
/// référence : une constante est recopiée à chaque emploi, donc elle n'a pas
/// d'adresse à emprunter.
static FIGURES: [Figure; 3] = [
    Figure {
        name: "d1",
        idle: include_bytes!("../assets/sprites/demon-d1-idle-8x8-64.png"),
        walk: include_bytes!("../assets/sprites/demon-d1-walk-8x8-64.png"),
        dead: include_bytes!("../assets/sprites/demon-d1-mort-8x16-64.png"),
        idle_margin: 3.0,
        walk_margin: 4.0,
        dead_margin: 2.0,
        stride: 36.0,
        bounty: 100,
    },
    Figure {
        name: "d2",
        idle: include_bytes!("../assets/sprites/demon-d2-idle-8x8-64.png"),
        walk: include_bytes!("../assets/sprites/demon-d2-walk-8x8-64.png"),
        dead: include_bytes!("../assets/sprites/demon-d2-mort-8x16-64.png"),
        idle_margin: 2.0,
        walk_margin: 3.0,
        dead_margin: 2.0,
        stride: 16.0,
        bounty: 200,
    },
    Figure {
        name: "d3",
        idle: include_bytes!("../assets/sprites/demon-d3-idle-8x8-64.png"),
        walk: include_bytes!("../assets/sprites/demon-d3-walk-8x8-64.png"),
        dead: include_bytes!("../assets/sprites/demon-d3-mort-8x16-64.png"),
        idle_margin: 3.0,
        walk_margin: 2.0,
        dead_margin: 3.0,
        stride: 22.0,
        bounty: 150,
    },
];

/// Les demi-étendues du **corps**, en unités de monde.
///
/// **Plus étroit que le joueur et de même hauteur** : il faut qu'il passe dans un
/// couloir et sous une porte, et c'est tout ce que le volume décide. Le brief de
/// l'étape borne un démon à `1,5` sur les deux axes horizontaux et `1,625` en
/// hauteur, donc il y a de la marge.
pub const HALF: Vec3 = Vec3::new(0.35, 0.35, 0.9);

/// La demi-étendue du **sprite**, en unités de monde et jamais en texels.
///
/// **Elle se règle à l'écran, et c'est la raison d'être de ce lot.** En texels, la
/// créature changerait de taille avec la résolution interne, qui est un réglage ;
/// en unités de monde, elle garde sa place dans le décor. La vignette étant carrée
/// et son cadrage identique d'une vue à l'autre, une seule demi-étendue sert les
/// deux axes et le quadrilatère ne se déforme pas quand il tourne.
///
/// Un peu moins d'un mètre : la vignette couvre donc près de deux mètres de haut,
/// ce qu'un démon qui dépasse l'homme demande une fois le cadrage retranché.
const SPRITE_HALF: f32 = 0.96;

/// Le rayon de la tache d'ombre, en unités de monde.
///
/// **Elle doit couvrir l'empreinte des pieds, et c'est ce qui la dimensionne.**
/// Relevée sur les six planches, cette empreinte atteint quarante-deux texels, soit
/// `1,26` de large ; une tache d'un mètre de diamètre y laissait les pieds dépasser,
/// et un pied hors de son ombre se lit comme un pied en l'air. C'est ce qui faisait
/// croire à un flottement, là où la mesure dans l'image montrait le bas du dessin
/// exactement sur la ligne du sol.
///
/// **Une seule tache pour les trois silhouettes, taillée sur la plus large.** Les
/// deux autres chaussent trente-trois et trente texels, donc leur tache dépasse de
/// dix-huit centimètres de chaque côté — et ne se voit pas, le dégradé s'y éteignant
/// déjà. Une tache par silhouette ferait trois textures pour un bord qu'on ne
/// distingue pas.
///
/// **Le dégradé fait le reste** : la tache est dense au centre et blanche au bord,
/// donc l'élargir ne la fait pas déborder sur les dalles voisines — elle s'y éteint.
const SHADOW_RADIUS: f32 = 0.68;

/// De combien la tache flotte au-dessus du sol, en unités de monde.
///
/// **Un décalage de géométrie et non un biais de profondeur**, et la nuance est
/// celle que le contrat pose : une tache posée dans le plan de la surface qu'elle
/// marque la marque, le test de profondeur tolérant la pente. Ce centimètre ne sert
/// donc pas à gagner le test — il sert à ne pas entrer dans la dalle quand le sol
/// est en pente sous la créature, le quadrilatère étant horizontal et la rampe non.
const SHADOW_LIFT: f32 = 0.01;

/// Ce que la tache laisse passer en son centre, sur 255.
///
/// **255 est le neutre de la modulation**, donc un texel blanc laisse le sol intact
/// et un texel noir l'éteint : une surface modulée assombrit ou ne fait rien, elle
/// n'éclaircit jamais. Un peu plus du cinquième ne noircit pas la dalle, ce qui
/// ferait un trou là où on veut un appui.
const SHADOW_CORE: f32 = 0x38 as f32;

/// Le côté de la texture de la tache, en texels.
///
/// **Une puissance de deux, que le moteur exige**, et petite : c'est un dégradé
/// radial, donc elle n'a aucun détail à porter et son filtrage fait le reste.
const SHADOW_SIDE: u32 = 64;

/// Le côté de cette texture en coordonnées de texture, qui se prennent en texels.
const SIDE: f32 = SHADOW_SIDE as f32;

/// Combien de temps un cycle de repos met à se jouer, en secondes.
///
/// **Le repos avance avec l'horloge, là où une marche avancera avec la distance
/// parcourue** : une créature immobile n'a aucune distance à offrir, et son cycle
/// doit pourtant tourner. Une seconde et demie pour huit trames donne une
/// respiration lente, qu'on ne confond pas avec un pas.
const IDLE_PERIOD: f32 = 1.5;

/// La vitesse de marche de la créature, en unités de monde par seconde.
///
/// **Plus lente que le joueur, qui va à 3,2** : une créature qui avance aussi vite
/// que celui qui la fuit ne se distingue pas d'un mur qui le suit.
///
/// **La même pour les trois, et c'est la foulée qui fait la différence** : à vitesse
/// égale, une silhouette à la foulée courte joue plus de cycles par seconde, donc
/// elle trottine là où une autre marche. C'est ce que les planches disent, et leur
/// donner une vitesse chacune pour égaliser la cadence reviendrait à corriger la
/// planche par le déplacement.
const SPEED: f32 = 1.4;

/// En deçà de quelle part du pas demandé la créature se tient pour arrêtée.
///
/// **Le critère porte sur la distance et jamais sur un axe.** Le balayage rend la
/// surface de **moindre pénétration**, donc l'axe qui arrête n'est pas celui qu'on
/// croit : une créature poussée contre un mur de face peut se voir arrêtée par le
/// sol, et une décision conditionnée à l'axe du cap ne se déclencherait jamais —
/// elle dériverait le long de la paroi en avançant toujours.
///
/// **Le seuil se mesure sur la glissade la plus favorable, pas au jugé.** Un cap à
/// quarante-cinq degrés contre une paroi axiale garde `cos 45°`, soit `0,707`, de
/// son pas : sous ce chiffre, la créature longe les murs indéfiniment sans être
/// retournée — relevé, neuf cents images sans un seul demi-tour. Au-dessus, elle se
/// retourne dès qu'elle perd franchement sa route, et continue de longer tant
/// qu'elle ne perd presque rien.
const STALLED: f32 = 0.8;

/// Combien d'images de suite la créature doit être gênée avant de se retourner.
///
/// **Une image gênée ne vaut pas un obstacle**, et c'est ce qui se voyait à
/// l'écran : en ligne droite, le franchissement d'un joint de dalle ou la reprise
/// d'une pente coûte un pas isolé, et la créature pivotait sans rien avoir
/// rencontré. Un mur, lui, gêne toutes les images jusqu'à ce qu'on s'en détourne.
///
/// **Mesuré sur une marche de quinze secondes** : le décor produit des séries
/// d'images gênées qui vont jusqu'à **six**, sans qu'aucun mur soit en cause — un
/// joint de dalle, la reprise d'une pente. Six était donc exactement le seuil que
/// la marche normale atteint, et la créature pivotait sur un accroc.
///
/// Le double, soit un cinquième de seconde : au-delà de ce que le décor produit en
/// ligne droite, et bien en deçà de ce qu'un mur impose — contre une paroi, la gêne
/// ne cesse pas tant qu'on ne s'en détourne pas.
const PATIENCE: u32 = 12;

/// Combien de coups une créature encaisse avant de tomber.
///
/// **Trois, et ce n'est pas un chiffre de goût** : au premier coup, le recul ne se
/// verrait jamais puisqu'elle mourrait à l'impact — les points de vie et la
/// rétroaction sont le même sujet. La cadence étant celle du doigt, le clic se lisant
/// en front et non en maintien, trois coups se tirent en une demi-seconde.
///
/// **Une constante et non un champ par silhouette** : la table serait l'endroit
/// naturel pour que les trois diffèrent, mais rien ne le demande encore, et un champ
/// portant trois fois la même valeur est une abstraction bâtie sur aucun cas.
const LIFE: u32 = 3;

/// Combien de temps le cycle de mort met à se jouer, en secondes.
///
/// **Il avance avec l'horloge, comme le repos et non comme la marche** : un cadavre ne
/// parcourt aucune distance, et un cycle indexé sur elle resterait figé sur sa première
/// trame.
///
/// **Une seconde et deux dixièmes pour seize trames, réglé à l'écran** : à huit, la
/// chute passait trop vite pour se lire — et c'est le seul moment où la silhouette se
/// donne à voir en entier, puisque rien ne reste ensuite.
const DEAD_PERIOD: f32 = 1.2;

/// La vitesse d'un recul, en unités de monde par seconde.
///
/// **Quatre fois la marche, et c'est ce qui le rend lisible** : à la vitesse de
/// promenade, un recul de trois images parcourrait sept centimètres, soit moins que le
/// pas que la créature fait de toute façon — on ne distinguerait pas un coup porté d'un
/// coup manqué, ce que l'étape existe précisément pour montrer.
const RECOIL_SPEED: f32 = 4.0 * SPEED;

/// Combien de temps un recul dure, en secondes.
///
/// **Trois images à soixante par seconde**, comme l'éclair de la pose de tir, et les
/// deux se répondent : ce que le joueur voit est une saccade de l'arme et une saccade
/// de la créature dans le même temps. Plus long, le recul deviendrait une poussée et la
/// créature paraîtrait glisser.
///
/// Avec la vitesse ci-dessus, cela fait **vingt-huit centimètres** — assez pour se lire
/// à quelques mètres, assez peu pour ne pas déplacer la créature d'une case.
const RECOIL_TIME: f32 = 0.05;

/// Le recul qu'un coup a imprimé à une créature.
///
/// **Un état et non un pas de plus**, et c'est tout ce que le lot a de difficile : le
/// déplacement de la marche est lié au cap, au critère d'arrêt et au cycle, et un recul
/// qui passerait par lui les fausserait tous les trois — la créature cesserait de se
/// détourner d'un mur, pivoterait parce qu'on lui a tiré dessus, et avancerait ses
/// jambes en étant poussée en arrière.
#[derive(Clone, Copy)]
struct Recoil {
    /// Sa vitesse, en unités de monde par seconde, horizontale.
    speed: Vec3,
    /// Ce qu'il lui reste à courir, en secondes.
    left: f32,
}

/// Une créature posée dans le décor.
///
/// **Un état de partie**, comme le joueur et l'arme : elle est jetée au
/// rechargement de la carte, et aucun de ses champs n'a sa place dans une structure
/// du monde.
pub struct Monster {
    /// La silhouette dont elle tient ses planches et ses cotes.
    figure: &'static Figure,
    /// Sa planche de repos, chargée une fois.
    idle: Arc<Texture>,
    /// Celle de sa marche.
    walk: Arc<Texture>,
    /// Celle de sa mort.
    dead: Arc<Texture>,
    /// La tache qu'elle pose au sol, partagée par toute la population.
    ///
    /// **Une seule pour les trois**, ce que la population a tranché comme prévu : le
    /// dégradé est le même sous les trois silhouettes, donc trois textures
    /// identiques n'auraient apporté que trois fois la mémoire.
    shadow: Arc<Texture>,
    /// Son corps, qui porte sa pose et sa cellule.
    ///
    /// **Le même chemin que le joueur**, et c'est ce que l'extraction du corps a
    /// acheté : la pose se résout par un balayage, quelle que soit la forme du sol
    /// sous elle, et le gabarit n'est plus celui d'un autre.
    body: Body,
    /// Son cap, en radians, qui décide de la vue avec la pose de l'œil.
    facing: f32,
    /// Ce qu'elle est en train de faire.
    motion: Motion,
    /// Où en est son cycle, en tours.
    phase: f32,
    /// Depuis combien d'images de suite le décor la gêne.
    ///
    /// **Remis à zéro dès qu'un pas passe**, ce qui est tout l'objet : ce qu'on
    /// cherche est un obstacle qui dure, pas un accroc isolé.
    hindered: u32,
    /// Le recul en cours, s'il y en a un.
    recoil: Option<Recoil>,
    /// Ce qu'elle peut encore encaisser, ou zéro si elle est tombée.
    ///
    /// **Ici et non dans la table des silhouettes** : c'est un état de partie, jeté au
    /// rechargement de la carte, là où la table porte des cotes de planche qui ne
    /// changent jamais.
    life: u32,
}

impl Monster {
    /// Pose une créature de cette silhouette debout sur cette case.
    ///
    /// **Le cap vient du rang et non d'une constante** : trois créatures au même cap
    /// dans le même couloir marchent en file et ne montrent qu'une seule de leurs
    /// huit vues. Réparties sur le tour, elles en montrent trois dès la première
    /// image — ce que ce lot existe pour donner à voir.
    ///
    /// # Erreurs
    ///
    /// Si une planche ne se décode pas — elles sont intégrées au binaire, donc jamais
    /// en pratique, mais un `expect` sur un chemin atteignable n'a pas sa place.
    fn new(
        figure: &'static Figure,
        shadow: Arc<Texture>,
        rank: usize,
        grid: &Grid,
        map: &World,
        at: (u32, u32, u32),
    ) -> Result<Self, Error> {
        Ok(Self {
            figure,
            idle: Arc::new(load_png_masked(figure.idle)?),
            walk: Arc::new(load_png_masked(figure.walk)?),
            dead: Arc::new(load_png_masked(figure.dead)?),
            shadow,
            body: Body::stand(HALF, grid, map, at),
            facing: rank as f32 * core::f32::consts::TAU / FIGURES.len() as f32,
            motion: Motion::Idle,
            phase: 0.0,
            hindered: 0,
            recoil: None,
            life: LIFE,
        })
    }

    /// Le pas que son cap lui demande pour cette image, avant tout obstacle.
    fn step(&self, dt: f32) -> Vec3 {
        Vec3::new(self.facing.cos(), self.facing.sin(), 0.0) * (SPEED * dt)
    }

    /// Vrai si ce pas la porterait dans le volume d'une autre créature.
    ///
    /// **Son propre rang s'écarte par l'indice et non par la distance** : une boîte
    /// recouvre toujours la sienne, et un écart nul ne se distingue pas du cas qu'on
    /// cherche.
    fn crowded(&self, crowd: &[Monster], rank: usize, dt: f32) -> bool {
        let here = self.body.centre();
        let wanted = here + self.step(dt);

        crowd.iter().enumerate().any(|(other, monster)| {
            // **Une créature tombée n'encombre plus** : elle est au sol, et s'arrêter
            // devant un cadavre resté debout en volume ferait buter les vivantes sur
            // rien de visible.
            if other == rank || monster.fallen() {
                return false;
            }
            let there = monster.body.centre();

            // **Un pas qui éloigne passe, même s'il laisse un recouvrement**, et c'est
            // la clause qui empêche l'enchevêtrement définitif : refuser tout pas dont
            // l'arrivée recouvre encore, c'est refuser aussi ceux qui en sortent —
            // vingt-huit centimètres de recul s'évacuent par une douzaine de pas de
            // deux centimètres et demi, dont pas un seul ne passerait. Deux créatures
            // bloquées l'une dans l'autre ne repartaient jamais, et le demi-tour de la
            // patience n'y changeait rien, le recouvrement étant le même des deux
            // côtés.
            //
            // **Le cas courant ne bouge pas** : hors recouvrement, un pas qui en crée un
            // rapproche forcément, donc il reste refusé.
            meets(wanted, there) && !recedes(here, wanted, there)
        })
    }

    /// Marche droit devant, et fait demi-tour quand le décor l'arrête.
    ///
    /// **Le déplacement passe par le corps**, avec son propre gabarit : la glissade,
    /// la chute et le franchissement sont ceux du joueur, et c'est ce que
    /// l'extraction du corps a acheté. Une créature monte donc un escalier sans
    /// qu'une ligne d'ici le sache.
    ///
    /// **Arrêtée veut dire demi-tour, quel que soit l'axe qui a arrêté**, et c'est le
    /// piège de cette fonction : le balayage rend la surface de moindre pénétration,
    /// donc la normale qui arrête n'est pas forcément celle du cap. Le critère porte
    /// sur la **distance horizontale réellement franchie** contre celle demandée —
    /// voir [`STALLED`].
    ///
    /// **La composante verticale ne compte pas** dans ce critère : la pesanteur en
    /// ajoute une à chaque image, et une créature qui descend une rampe parcourt plus
    /// que son pas horizontal sans avoir rien franchi de neuf.
    ///
    /// **Ce qu'elle coûte, vu à l'écran et assumé : la créature marche parfois en
    /// crabe.** La vue vient du **cap**, le déplacement de ce que la glissade a
    /// laissé passer ; en longeant une paroi de biais, les deux divergent et la
    /// silhouette avance de travers. Faire suivre la vue au déplacement réel la
    /// ferait pivoter à chaque frottement, ce qui se verrait davantage — et faire
    /// suivre le cap au déplacement reviendrait à longer les murs indéfiniment, ce
    /// que [`STALLED`] existe précisément pour empêcher.
    ///
    /// **Une autre créature sur le chemin annule le pas, elle ne le dévie pas.** Le
    /// pas nul épuise la patience comme une paroi le ferait, donc la créature
    /// s'arrête un cinquième de seconde puis se détourne — aucune politique de plus
    /// n'était à écrire. La chute, elle, continue de s'appliquer : [`Body::advance`]
    /// la porte, et une créature gênée par une autre au bord d'un palier doit tomber
    /// comme les autres.
    fn walk(&mut self, map: &World, dt: f32, crowded: bool) {
        // **Une créature tombée ne marche plus, mais elle tombe encore** : le corps
        // reçoit un pas nul, donc la pesanteur s'applique et un cadavre abattu au bord
        // d'un palier rejoint le sol. Sa phase avance avec l'horloge, puisqu'il ne
        // parcourt aucune distance, et le cycle de mort ne boucle pas — elle reste sur
        // sa dernière trame.
        if self.fallen() {
            self.body.advance(map, Vec3::new(0.0, 0.0, 0.0), dt);
            self.phase += dt / DEAD_PERIOD;
            return;
        }

        if self.recoiling(map, dt) {
            return;
        }

        let step = self.step(dt);
        // **Le pas demandé reste celui du cap, même annulé**, et c'est ce qui fait
        // tomber le critère du bon côté : comparé à zéro, un pas nul paraîtrait
        // franchi en entier, la patience se remettrait à zéro, et deux créatures se
        // pousseraient indéfiniment sans jamais se détourner.
        let moved = match crowded {
            true => Vec3::new(0.0, 0.0, 0.0),
            false => step,
        };

        let before = self.body.centre();
        self.body.advance(map, moved, dt);
        let after = self.body.centre();

        let gone = Vec3::new(after.x - before.x, after.y - before.y, 0.0);
        let wanted = step.dot(step);
        self.hindered = match gone.dot(gone) < wanted * STALLED * STALLED {
            true => self.hindered + 1,
            false => 0,
        };
        if self.hindered >= PATIENCE {
            self.facing += core::f32::consts::PI;
            self.hindered = 0;
        }

        self.advance(gone.dot(gone).sqrt(), dt);
    }

    /// Imprime un recul dans cette direction, et rend la main au coup suivant.
    ///
    /// **La direction est ramenée à l'horizontale**, et c'est une règle de jeu : un tir
    /// en plongée soulèverait la créature ou l'enfoncerait dans le sol, là où ce qu'on
    /// veut montrer est qu'elle encaisse. Une direction purement verticale n'imprime
    /// donc rien, ce qui est la seule réponse sensée — il n'y a pas d'horizontale à en
    /// tirer.
    ///
    /// **Un coup pendant un recul le relance**, il ne s'y ajoute pas : deux reculs
    /// composés doubleraient la vitesse, et la créature partirait d'un bond au second
    /// coup d'une rafale. C'est la même clause que la pose de tir de l'arme, qui se
    /// relance plutôt que de s'accumuler.
    ///
    /// **Rend sa prime pour le seul coup qui l'abat**, et c'est ce que le score
    /// compte : la vie n'atteint zéro qu'ici, une fois, et les coups sur une tombée ne
    /// rendent rien comme ils ne font rien. Compter au retrait des mortes, par
    /// différence de longueur, arriverait une seconde et deux dixièmes plus tard — le
    /// temps de sa chute — et serait faux le jour où une créature part pour une autre
    /// raison.
    ///
    /// **La prime et non un simple oui**, parce qu'elle appartient à la silhouette :
    /// la faire lire au tir par un second appel laisserait à l'appelant le soin de
    /// savoir quand la lire, ce que seul ce retour sait.
    pub fn knock(&mut self, push: Vec3) -> Option<u32> {
        if self.fallen() {
            return None;
        }

        self.life -= 1;
        if self.fallen() {
            // **Le coup fatal ne recule pas**, et c'est ce qui le distingue des
            // autres : la créature tombe là où elle est, et sa chute est la
            // rétroaction. Un recul par-dessus la ferait glisser en tombant.
            self.motion = Motion::Dead;
            self.phase = 0.0;
            self.recoil = None;
            return Some(self.figure.bounty);
        }

        let flat = Vec3::new(push.x, push.y, 0.0);
        let length = flat.dot(flat).sqrt();
        if length == 0.0 {
            return None;
        }

        self.recoil = Some(Recoil {
            speed: flat * (RECOIL_SPEED / length),
            left: RECOIL_TIME,
        });
        None
    }

    /// Applique le recul en cours, et dit s'il a pris la main sur ce pas.
    ///
    /// **Ce qu'il court-circuite est le fond du sujet**, et chacun des trois pour une
    /// raison mesurée :
    ///
    /// - **le cap**, parce qu'une créature poussée ne marche pas — elle est déplacée ;
    /// - **le critère d'arrêt**, qui compare la distance franchie au pas demandé par le
    ///   cap : un recul le satisferait largement et remettrait la patience à zéro, donc
    ///   une créature reculée contre un mur cesserait de s'en détourner ; à l'inverse,
    ///   un recul opposé au cap annulerait le pas franchi et l'épuiserait, et la
    ///   créature pivoterait parce qu'on lui a tiré dessus ;
    /// - **le cycle**, qui se choisit sur la distance parcourue : le recul en produit,
    ///   donc la créature avancerait ses jambes en partant en arrière. La foulée valant
    ///   de quarante-huit centimètres à un mètre selon la silhouette, les vingt-huit
    ///   centimètres d'un recul feraient défiler une fraction de pas bien visible.
    ///
    /// **Ce qu'il garde, en revanche, c'est le décor** : le déplacement passe par le
    /// corps, donc le balayage l'arrête et la glissade s'applique. Une créature reculée
    /// contre une paroi s'y tasse au lieu de la traverser.
    ///
    /// **Et il ignore le voisinage**, là où la marche l'évite : le coup l'emporte sur la
    /// politique d'évitement, donc un recul peut enfoncer une créature dans une autre.
    ///
    /// **Ce qu'elles font ensuite est l'affaire de [`Monster::crowded`]**, et ce n'était
    /// pas gratuit : il refusait tout pas dont l'arrivée recouvrait encore, donc elles
    /// restaient enchevêtrées pour toujours — vu à l'écran, et corrigé là-bas en
    /// laissant passer ce qui éloigne. Ce texte affirmait qu'elles « se séparent
    /// d'elles-mêmes au pas suivant » : c'était faux.
    fn recoiling(&mut self, map: &World, dt: f32) -> bool {
        let Some(recoil) = self.recoil.as_mut() else {
            return false;
        };

        // Le dernier pas est tronqué à ce qu'il reste, pour que la distance parcourue ne
        // dépende pas de la cadence : à trente images par seconde comme à cent vingt, le
        // recul vaut sa vitesse fois sa durée.
        let step = dt.min(recoil.left);
        let moved = recoil.speed * step;
        recoil.left -= step;
        if recoil.left <= 0.0 {
            self.recoil = None;
        }

        self.body.advance(map, moved, dt);
        true
    }

    /// Avance son cycle de ce qu'elle a parcouru, et du temps écoulé.
    ///
    /// **C'est la distance qui décide du cycle** : une créature qui marche en
    /// montrant des poses de repos est précisément ce qu'on ne verrait pas tout de
    /// suite, et un cycle indexé sur l'horloge continuerait de défiler contre un mur.
    ///
    /// **Changer de cycle remet la phase à zéro**, parce que les deux planches n'ont
    /// pas le même compte de trames et qu'une phase reportée tomberait au milieu
    /// d'un pas. Une créature qui s'arrête repart de sa première pose de repos.
    fn advance(&mut self, travel: f32, dt: f32) {
        let motion = match travel > 0.0 {
            true => Motion::Walk,
            false => Motion::Idle,
        };
        if motion != self.motion {
            self.motion = motion;
            self.phase = 0.0;
        }

        self.phase += match motion {
            Motion::Walk => travel / self.figure.stride(),
            _ => dt / IDLE_PERIOD,
        };
    }

    /// La planche que son mouvement courant lui donne.
    fn sheet(&self) -> &Arc<Texture> {
        match self.motion {
            Motion::Walk => &self.walk,
            Motion::Dead => &self.dead,
            Motion::Idle => &self.idle,
        }
    }

    /// Vrai si elle est tombée, donc si elle ne marche plus et n'est plus touchable.
    pub fn fallen(&self) -> bool {
        self.life == 0
    }

    /// Vrai quand sa chute est jouée en entier, donc qu'il n'y a plus rien à montrer.
    ///
    /// **Le cycle de mort ne boucle pas**, et c'est ce qui donne le repère : passé le
    /// tour, la colonne reste sur la dernière trame, donc une phase qui l'atteint dit
    /// que tout a été vu. La créature quitte alors la population — rien ne reste au
    /// sol, pas même son ombre, qui s'est éteinte dès le coup fatal.
    pub fn spent(&self) -> bool {
        self.fallen() && self.phase >= 1.0
    }

    /// Où son centre se trouve, ce que le titre de la fenêtre affiche.
    pub fn at(&self) -> Vec3 {
        self.body.centre()
    }

    /// Le nom de sa silhouette, que le titre affiche avec sa distance.
    pub fn name(&self) -> &'static str {
        self.figure.name
    }
}

/// Vrai si ce pas éloigne du point donné.
///
/// **Les carrés se comparent sans racine** : ce qui est demandé est un ordre, pas une
/// distance, et la racine est monotone.
///
/// **Strictement, et c'est une prudence et non une mesure** : un pas qui garde l'écart
/// ne sort de rien, donc il n'a pas de raison de passer. L'égalité à la place n'a fait
/// bouger aucune épreuve — deux créatures de même cap se séparent par d'autres voies,
/// le décor ne les arrêtant pas au même instant —, donc ce `>` ne se prévaut d'aucun cas
/// qu'il serait seul à traiter.
fn recedes(here: Vec3, wanted: Vec3, from: Vec3) -> bool {
    let (before, after) = (here - from, wanted - from);
    after.dot(after) > before.dot(before)
}

/// Vrai si deux corps centrés là se recouvrent.
///
/// **Le gabarit est celui du module et non un champ du corps** : toutes les
/// créatures le partagent, et une boîte par silhouette n'aurait pas de mesure pour
/// la justifier — le volume tient à ce qui doit passer dans un couloir, pas au
/// dessin.
///
/// **Ce qui reste ici est donc le gabarit, pas le test** : l'inégalité vit auprès de
/// la boîte depuis que le joueur en demande une seconde avec des gabarits inégaux.
fn meets(here: Vec3, there: Vec3) -> bool {
    body::overlaps(here, HALF, there, HALF)
}

/// Les cases où les créatures naissent : les plus proches de l'entrée par les
/// passages.
///
/// **Un parcours en largeur et non une distance de grille** : deux cases voisines
/// peuvent être séparées par un mur, donc proches sans être en vue. Ce qui compte
/// est la proximité **par les couloirs**, et c'est ce qu'un parcours mesure.
///
/// **Elles sont dans le champ au lancement, et c'est ce que ce lot demande** : trois
/// silhouettes dispersées sur cinq cents cases ne se comparent pas, et comparer les
/// trois planches est précisément ce qui se juge ici. Elles se séparent ensuite
/// d'elles-mêmes en marchant.
///
/// **Les passages verticaux sont écartés** : une créature née dans une cage serait
/// hors de vue au lancement, et c'est tout — sa pose, elle, se résout comme une
/// autre, le relèvement de pente étant l'affaire de [`Body::stand`].
///
/// Rend moins de cases que de silhouettes si le labyrinthe n'en offre pas assez, ce
/// qu'aucune taille jouable ne produit ; l'appelant retombe alors sur le départ
/// plutôt que d'ériger en invariant une commodité de placement.
fn spots(grid: &Grid) -> Vec<(u32, u32, u32)> {
    let mut queue = VecDeque::from([grid.start()]);
    let mut seen = vec![grid.start()];
    let mut found = Vec::with_capacity(FIGURES.len());

    while let Some(at) = queue.pop_front() {
        for side in Side::ALL.into_iter().filter(|side| !side.is_vertical()) {
            if found.len() == FIGURES.len() {
                return found;
            }
            if grid.has_wall(at, side) {
                continue;
            }
            let Some(next) = grid.neighbour(at, side) else {
                continue;
            };
            if seen.contains(&next) {
                continue;
            }
            seen.push(next);
            queue.push_back(next);
            found.push(next);
        }
    }

    found
}

/// Peuple le labyrinthe de ses trois silhouettes.
///
/// **La tache d'ombre se fabrique ici, une fois pour toutes** : c'était la décision
/// que ce lot devait prendre, et le partage par `Arc` la rend sans coût.
///
/// # Erreurs
///
/// Si une planche ne se décode pas — elles sont intégrées au binaire, donc jamais en
/// pratique.
pub fn population(grid: &Grid, map: &World) -> Result<Vec<Monster>, Error> {
    let shadow = Arc::new(shadow_texture());
    let spots = spots(grid);
    let start = grid.start();

    FIGURES
        .iter()
        .enumerate()
        .map(|(rank, figure)| {
            let at = spots.get(rank).copied().unwrap_or(start);
            Monster::new(figure, Arc::clone(&shadow), rank, grid, map, at)
        })
        .collect()
}

/// Fait marcher la population : chacune contre le décor, et contre les autres.
///
/// **Qu'une créature en arrête une autre est une règle du jeu**, et il fallait
/// l'écrire : le moteur n'arrête que la géométrie de cellule, un démon n'a ni
/// portail ni adjacence, donc deux silhouettes se traversaient sans qu'aucune clause
/// soit en défaut. C'est la même frontière que pour le tir.
///
/// **Elles se séparent sur leur volume et non sur leur dessin.** Deux vignettes font
/// près de quatre unités de large, soit plus qu'un couloir n'en laisse : séparées
/// ainsi, deux créatures s'y coinceraient pour de bon, chacune empêchant l'autre de
/// passer. Elles se frôlent donc, et leurs dessins se chevauchent — ce que le
/// z-tampon départage, et le volume n'est pas le dessin.
///
/// **Chacune décide contre les poses déjà avancées de cette image**, l'ordre de la
/// liste faisant foi. Les faire toutes décider contre la pose d'avant laisserait deux
/// créatures se croiser dans le même pas, qui est précisément le cas qu'on écarte.
pub fn stroll(monsters: &mut [Monster], map: &World, dt: f32) {
    for rank in 0..monsters.len() {
        let crowded = monsters[rank].crowded(monsters, rank, dt);
        monsters[rank].walk(map, dt, crowded);
    }
}

/// La texture de la tache d'ombre : sombre au centre, **blanche au bord**.
///
/// **Le disque vient de [`crate::blot`] depuis que la marque d'impact en veut un
/// aussi** : les deux ne diffèrent que par le côté et la densité, et ce module dit
/// pourquoi il est blanc au bord plutôt que transparent, et pourquoi il s'engendre
/// plutôt que de se charger. Ce qui reste ici est le choix des deux cotes.
fn shadow_texture() -> Texture {
    blot(SHADOW_SIDE, SHADOW_CORE)
}

/// Les quatre coins de la tache d'une créature, dans le sens qui la rend visible.
///
/// **Le sens décide de la face vue**, et c'est le piège du projet : la caméra
/// neutre regarde le +X, son axe droit est le −Y et son haut le +Z. Décrit dans
/// l'autre sens, le quadrilatère est un dos de face et disparaît — sans erreur, et
/// sans rien à l'écran.
fn shadow_corners(centre: Vec3) -> [Vec3; 4] {
    let z = centre.z - HALF.z + SHADOW_LIFT;
    let corner = |dx: f32, dy: f32| Vec3::new(centre.x + dx, centre.y + dy, z);

    [
        corner(-SHADOW_RADIUS, -SHADOW_RADIUS),
        corner(SHADOW_RADIUS, -SHADOW_RADIUS),
        corner(SHADOW_RADIUS, SHADOW_RADIUS),
        corner(-SHADOW_RADIUS, SHADOW_RADIUS),
    ]
}

/// Où poser le centre du quadrilatère pour que les pieds touchent le sol.
///
/// **Ce n'est pas le centre du corps**, et c'est ce que la fonction existe pour
/// dire : le dessin monte plus haut que la boîte, et la planche laisse du vide
/// sous les pieds. Le centre monte donc de la demi-étendue du sprite, moins ce que
/// le cadrage a laissé — un retrait qui est la même fraction de la vignette en
/// unités de monde qu'en texels.
fn anchor(centre: Vec3, figure: &Figure, motion: Motion) -> Vec3 {
    let lift = SPRITE_HALF - figure.margin(motion) / FRAME * (2.0 * SPRITE_HALF);

    Vec3::new(centre.x, centre.y, centre.z - HALF.z + lift)
}

/// Soumet les créatures, **après le décor et avant l'arme**.
///
/// **Les taches d'abord, et c'est la seule contrainte d'ordre qui reste.** Une
/// surface modulée multiplie le tampon : elle ne peut assombrir que ce qui y est
/// déjà, donc le décor doit avoir été soumis. Elle teste la profondeur sans
/// l'écrire, et son test tolère la pente, si bien qu'elle gagne sur les dalles par
/// le seul ordre de soumission — sans biais de profondeur, qui vaudrait des
/// millimètres de près et des mètres au loin.
///
/// **Les sprites, eux, ne s'ordonnent pas** : la transparence du moteur est binaire,
/// donc le z-buffer tranche dans n'importe quel ordre et aucun tri n'est à faire. Ce
/// que leur ordre évite est qu'un décor très proche les rejette à égalité, et l'arme
/// reste la dernière parce qu'elle est la plus proche de l'œil.
///
/// **Le centre du quadrilatère n'est pas celui du corps.** Les pieds doivent
/// toucher le sol, et la planche laisse du vide sous eux : le centre monte donc de
/// la demi-étendue du sprite, moins ce que le cadrage a laissé. Poser le sprite sur
/// le centre du corps ferait flotter une créature haute et enterrerait une basse.
///
/// Comme la scène et l'arme, cette fonction ne lit ni horloge, ni entrée, ni tampon
/// de sortie : le chemin de rendu hors fenêtre de l'étape 8 l'appellera telle
/// quelle.
pub fn submit(
    context: &mut Context,
    monsters: &[Monster],
    camera: &Camera,
) -> Result<(), screengine_play::screengine::Error> {
    // **Une créature tombée ne pose plus d'ombre**, et dès le coup fatal : la tache est
    // dimensionnée sur l'empreinte des pieds d'une silhouette debout, et un disque resté
    // rond sous un corps qui s'affaisse se lit comme une marque au sol. Rien ne doit
    // rester d'un démon abattu.
    for monster in monsters.iter().filter(|monster| !monster.fallen()) {
        let corners = shadow_corners(monster.body.centre());
        let white = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
        let vertices: Vec<VertexUv> = corners
            .iter()
            .zip([(0.0, 0.0), (SIDE, 0.0), (SIDE, SIDE), (0.0, SIDE)])
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
            // Pas de mode à passer : `submit_blended` **est** la modulation côté
            // Rust, là où la frontière C prend un `blend` qui lui laisse la place
            // d'un mode additif. Rien à choisir ici, donc rien à se tromper.
            Some(&monster.shadow),
        )?;
    }

    for monster in monsters {
        let centre = monster.body.centre();
        let row = sheet::row(monster.facing, centre, camera.position);
        let column = sheet::column(monster.motion, monster.phase);
        let (u0, v0, u1, v1) = sheet::rect(row, column);

        context.submit_sprites(
            Affine3::IDENTITY,
            &[Sprite {
                center: anchor(centre, monster.figure, monster.motion),
                half_width: SPRITE_HALF,
                half_height: SPRITE_HALF,
                u0,
                v0,
                u1,
                v1,
                // Une créature debout ne roule pas : le roulis sert une lueur ou une
                // étincelle, et il tournerait ici la silhouette dans son plan.
                roll: Angle::from_radians(0.0),
                color: Color::new(0xFF, 0xFF, 0xFF, 0xFF),
            }],
            Some(monster.sheet()),
            SpriteOrientation::Axial,
        )?;
    }

    Ok(())
}
