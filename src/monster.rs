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

use std::sync::Arc;

use screengine_play::{
    Affine3, Angle, Camera, Color, Context, CoreError, Error, Sprite, SpriteOrientation, Surfaces,
    Sweep, Texture, Triangle, Vec3, VertexUv, World, load_png_masked,
};

use crate::blot::blot;
use crate::body::Body;
use crate::maze::export;
use crate::maze::grid::{Grid, Side};
use crate::rng::Rng;
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
    /// La largeur de son **corps** dessiné, en texels de planche.
    ///
    /// **Relevée et non choisie**, comme les marges et la foulée : c'est la médiane des
    /// largeurs de ligne de ses vignettes, prise au plus large de ses deux cycles
    /// vivants. `le_corps_annonce_est_celui_de_la_planche` la rattache aux planches, et
    /// une planche refaite doit donc faire rougir la table plutôt que de passer.
    ///
    /// **La médiane et pas l'enveloppe**, et l'écart est du double : `d2` est dessinée
    /// sur cinquante-sept texels avec ses appendices, et son corps en fait
    /// trente-sept. Un volume qui couvrirait l'enveloppe rendrait touchable le vide
    /// entre les membres ; ce qu'on cherche est ce qu'un joueur prend pour le corps.
    trunk: f32,
    /// Ce qu'elle encaisse de coups, avant la variation de chaque individu.
    ///
    /// **Elle suit le corps dessiné, et c'est la mesure qui l'a décidée** : `d2` fait
    /// trente-sept texels de tronc, `d1` trente et un, `d3` vingt-deux. Une résistance
    /// qui suit la corpulence se lit à l'écran sans qu'on l'explique, là où trois
    /// valeurs égales — ce que la constante portait — n'étaient une abstraction
    /// bâtie sur aucun cas.
    ///
    /// **Une base et non la valeur** : chaque créature la décale d'un coup au plus à
    /// sa naissance. La silhouette reste donc prédictive, ce qui est la condition
    /// pour qu'une résistance se lise — deux démons identiques qui tomberaient en
    /// deux coups et en cinq seraient la forme exacte du défaut de l'étape 5, une
    /// qualité invisible que le barème récompense.
    life: u32,
    /// Ce qu'elle vaut au compteur, abattue.
    ///
    /// **Une prime par silhouette et non un barème unique**, parce que ce qui la
    /// justifiera est déjà prévu : elles ne mourront pas de la même façon, et il
    /// faudra plus de coups pour certaines. Un barème commun serait à défaire ce
    /// jour-là, et la table est l'endroit où il s'écrit — tout ce qui sépare les
    /// trois y est déjà.
    ///
    /// **L'ordre est celui de la table, et il n'affirme rien** : les trois sont
    /// également touchables depuis que [`Figure::touch`] suit le corps dessiné, et elles
    /// partagent vie, vitesse et volume de marche. Rien ne les ordonne donc, et les
    /// valeurs se reprendront quand leur résistance divergera.
    ///
    /// **`d2` a valu le plus pendant un lot**, parce qu'elle paraissait la plus dure à
    /// abattre : elle l'était, mais d'un défaut — son corps dessiné dépassait de moitié
    /// son volume touchable. Le barème récompensait un ratage, et c'est ce qui a fait
    /// mesurer plutôt que payer.
    ///
    /// **Elles suivent désormais [`Figure::life`]**, ce que ce champ annonçait : la
    /// résistance a divergé, donc la prime se range dans son ordre. `d3` est passée du
    /// plus au moins payant en devenant la plus fragile — elle valait le plus sans
    /// qu'aucune mesure le dise.
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

    /// Les demi-étendues de ce que le **tir** peut toucher, en unités de monde.
    ///
    /// **Ce n'est pas [`HALF`], et c'est la correction de tout ce lot** : ce volume-là
    /// est le corps qui doit passer dans un couloir, et il vaut le même pour les trois.
    /// Celui-ci doit couvrir ce qu'on voit, parce qu'on vise ce qu'on voit — et le
    /// corps dessiné de `d2` fait `1,11` de large là où le volume de marche en fait
    /// `0,70`. Viser son flanc ratait le coup sans que rien ne le dise, ce qui s'est vu
    /// à l'écran et a d'abord été pris pour une difficulté de la créature.
    ///
    /// **Jamais sous le volume de marche** : `d3` mesure `0,66`, donc moins que les
    /// `0,70` d'aujourd'hui, et un correctif qui la rendrait plus dure à toucher ne
    /// serait pas un correctif. Le plancher est donc le gabarit de marche.
    ///
    /// **Carré en plan, comme [`HALF`]** : la planche est orientée caméra, donc la
    /// largeur dessinée est la même sous tous les angles. Une boîte plus profonde que
    /// large se laisserait toucher de côté à une distance qu'on ne voit pas.
    ///
    /// **La hauteur ne bouge pas, et c'est mesuré** : les trois silhouettes sont
    /// dessinées sur cinquante-neuf à soixante texels, soit `1,77` à `1,80`, là où le
    /// volume en fait `1,80`. Viser la tête touche déjà.
    fn touch(&self) -> Vec3 {
        let half = (self.trunk / FRAME * SPRITE_HALF).max(HALF.x);
        Vec3::new(half, half, HALF.z)
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
        trunk: 31.0,
        life: 3,
        bounty: 150,
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
        trunk: 37.0,
        life: 4,
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
        trunk: 22.0,
        life: 2,
        bounty: 100,
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

/// À quelle distance du centre d'une case la créature la tient pour atteinte.
///
/// **Un huitième de case, et ce n'est pas une tolérance de confort** : la créature
/// vise un point, et un seuil nul ne se franchit jamais — le pas vaut deux centimètres
/// et demi à soixante images par seconde, donc elle osciperait autour du centre sans
/// jamais y être. Ce qu'on veut est qu'elle engage le passage suivant, pas qu'elle
/// touche un point.
///
/// **Et le virage se coupe d'autant**, ce qui est voulu : tourner au centre exact
/// donnerait un angle droit que personne ne marche. La glissade du corps rattrape ce
/// que la coupe laisse passer.
const ARRIVED: f32 = export::CELL / 8.0;

/// À quelle distance une créature voit le joueur, en unités de monde.
///
/// **Quatre cases, et la cote se lit contre le brouillard** : il ferme la vue à
/// vingt-deux unités, et détecter plus loin qu'on ne voit serait injuste — une créature
/// arriverait de l'invisible. À seize, il laisse encore passer les deux tiers de la
/// lumière, donc ce qui vient a été vu avant d'arriver. C'est la même exigence que
/// l'étape 13 écrit pour les arrivées : se voir avant de se subir.
///
/// **Et le dégagement du semis la borne par le haut** : aucune créature ne naît à moins
/// de six cases du départ, soit vingt-quatre unités. Huit unités de marge, donc la
/// partie ne commence pas par une attaque — c'est la cote à relire si l'une des deux
/// bouge.
///
/// **Aucun angle ne s'y ajoute, et c'est tranché** : une créature détecte le joueur dans
/// son dos. Deux cotes inconnues à la fois ne se départagent pas à l'écran, et un cône
/// s'ajoute ensuite par un produit scalaire, le cap existant déjà. Ce qu'il faut guetter
/// en jouant : une créature qu'on suit dans le dos indéfiniment, qui se lirait comme un
/// défaut et non comme de la furtivité.
const SIGHT: f32 = 16.0;

// **Détecter plus loin que le brouillard ne laisse voir serait injuste** : une créature
// arriverait de l'invisible. Relation entre constantes, donc vérifiée ici et non dans
// une épreuve — et c'est le genre de couple qu'un réglage d'ambiance défait sans y
// penser, le brouillard se jugeant à l'œil.
const _: () = assert!(SIGHT < crate::scene::FOG_END);

/// Combien de temps elle continue vers le dernier endroit où elle a vu le joueur, en
/// secondes.
///
/// **Sans elle, se décrocher est gratuit** : un pas derrière un angle suffit à être
/// oublié, et la détection n'a alors aucun poids. Ce qu'on veut est que se défaire d'une
/// créature prenne un moment.
///
/// **Et elle n'est pas une poursuite hors de vue**, qui reste hors périmètre : la
/// créature va vers un **point**, pas vers le joueur. Elle n'en sait rien de plus que
/// l'endroit où il était, donc aucun chemin n'est à calculer et le joueur qui a tourné
/// deux fois est perdu pour de bon.
///
/// **Trois secondes, et la cote se lit contre la vitesse** : à `2,8`, elle couvre huit
/// unités, soit deux cases. De quoi franchir l'angle derrière lequel le joueur vient de
/// disparaître, pas de quoi traverser un étage.
const MEMORY: f32 = 3.0;

/// De combien elle presse le pas en poursuivant.
///
/// **Le double, et le chiffre se lit contre la vitesse du joueur** : il marche à `3,2`,
/// elle patrouille à `1,4`, soit quarante-quatre pour cent. En poursuite elle monte à
/// `2,8` — **toujours moins que lui**, donc fuir en ligne droite reste gagnant, mais
/// l'écart tombe de `1,8` à `0,4` et le moindre détour se paie. Une créature plus rapide
/// que le joueur retirerait la seule réponse qu'il a, et une qui reste à `1,4` ne
/// menace rien.
///
/// **Ce qui en fait une course et non une approche** : la planche de marche avance avec
/// la distance parcourue, donc le cycle s'accélère de lui-même et la poursuite se voit
/// sans qu'une seconde planche soit à produire.
const CHASE: f32 = 2.0;

// La poursuite reste sous la marche du joueur : sans cela, courir cesse d'être une
// réponse et il ne reste qu'à tirer. Relation entre constantes de deux modules, donc
// vérifiée ici — c'est le genre de couple qu'un réglage de confort défait sans y penser.
const _: () = assert!(SPEED * CHASE < crate::game::WALK);

/// De combien une créature serre à sa droite dans un passage, en unités de monde.
///
/// **Deux créatures ne peuvent pas viser le même point**, et c'est tout l'objet : au
/// milieu exact, celles qui se croisent convergent, chacune annule le pas de l'autre —
/// l'évitement étant symétrique —, et aucune ne cède. Mesuré sur la graine `0x1`, deux
/// silhouettes immobiles trois secondes durant, que le demi-tour de la patience
/// renvoyait converger dans l'autre sens.
///
/// **À droite du sens de marche, et non d'un côté tiré par créature.** Un décalage
/// propre à chacune ne sépare rien quand deux créatures se font face avec des valeurs
/// de même signe : leurs sens étant opposés, elles se rangent alors du même côté. La
/// droite, elle, est relative au trajet, donc deux sens contraires se rangent toujours
/// l'un en face de l'autre — c'est la règle de circulation, et elle n'a besoin d'aucun
/// état.
///
/// **Quatre dixièmes, et le passage le permet** : il fait trois unités de large, un
/// gabarit en occupe `0,70`, et le centre d'un corps peut s'écarter de `1,15` avant de
/// toucher la paroi. Deux trajectoires opposées se croisent donc à `0,80` d'écart pour
/// `0,70` nécessaires, en restant à `0,75` du milieu.
const DRIFT: f32 = 0.4;

/// Combien d'images de suite la créature doit ne plus approcher avant de rechoisir.
///
/// **Une image sans approcher ne vaut pas un obstacle**, et c'est ce qui se voyait à
/// l'écran du temps de l'errance : le franchissement d'un joint de dalle ou la reprise
/// d'une pente coûte un pas isolé, et la créature changeait d'avis sans rien avoir
/// rencontré. Une autre créature en travers, elle, dure jusqu'à ce qu'on s'en détourne.
///
/// **Mesuré sur une marche de quinze secondes** : le décor produit des séries d'images
/// gênées qui vont jusqu'à **six**, sans qu'aucun mur soit en cause. Six était donc
/// exactement le seuil que la marche normale atteint.
///
/// Le double, soit un cinquième de seconde : au-delà de ce que le décor produit en
/// ligne droite, et bien en deçà de ce qu'une créature arrêtée impose.
///
/// **Ce qu'elle garde a changé de nature avec la patrouille** : le décor ne la détourne
/// plus, puisqu'elle suit les passages au lieu de buter dessus. Ce qui reste à
/// débloquer est une autre créature en travers du couloir — l'évitement annule le pas,
/// et sans cette garde deux créatures face à face s'y tiendraient indéfiniment.
const PATIENCE: u32 = 12;

/// Combien de coups une créature encaisse avant de tomber.
///
/// De combien de coups la résistance d'un individu s'écarte de celle de sa silhouette.
///
/// **Un seul coup, et c'est la silhouette qui doit rester prédictive** : la base
/// porte ce qu'on lit — une créature massive encaisse plus —, et la variation ne fait
/// que l'empêcher d'être un barème. Plus large, elle rendrait la silhouette muette, ce
/// que `D31` écarte nommément.
///
/// **Et la base ne descend jamais sous deux**, donc un écart d'un coup laisse au moins
/// un coup à encaisser : le recul ne se verrait pas sur une créature qui tombe à
/// l'impact, et les points de vie et la rétroaction sont le même sujet.
const SPREAD: u32 = 1;

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
    /// Depuis combien d'images de suite elle n'approche plus de sa case visée.
    ///
    /// **Remis à zéro dès qu'elle gagne du terrain**, ce qui est tout l'objet : ce
    /// qu'on cherche est un obstacle qui dure, pas un accroc isolé.
    hindered: u32,
    /// La case qu'elle vise, et le côté de celle-ci par lequel elle y entrera.
    ///
    /// **La patrouille tient dans ces deux valeurs**, et rien d'autre n'est gardé : pas
    /// de chemin calculé d'avance, pas de file de cases. La créature va au centre d'une
    /// case voisine, et en choisit une autre en arrivant — ce qui suffit à suivre un
    /// couloir, et ce qui laisse le labyrinthe décider du trajet plutôt qu'une
    /// structure qu'il faudrait tenir à jour quand le décor change.
    ///
    /// **Le côté d'entrée est ce qui empêche le va-et-vient** : il s'exclut du choix
    /// suivant tant qu'une autre sortie existe, donc la créature traverse les couloirs
    /// au lieu d'osciller entre deux cases. Dans un cul-de-sac, il redevient éligible
    /// et c'est le demi-tour.
    goal: (u32, u32, u32),
    /// Par quel côté de [`Monster::goal`] elle y entre.
    entered: Side,
    /// Ce qu'elle poursuit : le dernier endroit où le joueur a été vu, et pour combien
    /// de temps encore.
    ///
    /// **Un point et un décompte, pas une cible** : elle ne sait rien du joueur au-delà
    /// de l'endroit où il était. C'est ce qui sépare cette mémoire d'une poursuite hors
    /// de vue, qui demanderait un chemin vers quelqu'un qui bouge — et c'est resté hors
    /// périmètre.
    ///
    /// **Rafraîchi tant qu'elle le voit**, puis épuisé par le temps. Il sert aussi à
    /// savoir quand reprendre la patrouille : sans lui, une créature qui n'a jamais vu
    /// personne rechoisirait sa sortie à chaque image et resterait au centre de sa case.
    hunt: Option<(Vec3, f32)>,
    /// Le recul en cours, s'il y en a un.
    recoil: Option<Recoil>,
    /// Ce qu'elle peut encore encaisser, ou zéro si elle est tombée.
    ///
    /// **Ici et non dans la table des silhouettes** : c'est un état de partie, jeté au
    /// rechargement de la carte, là où la table porte des cotes de planche qui ne
    /// changent jamais.
    life: u32,
}

/// Les trois planches d'une silhouette, décodées une fois pour toutes celles qui la
/// portent.
///
/// **Le partage se fait par silhouette et non par créature**, et c'est le compte qui
/// l'a imposé : seize créatures décodaient quarante-huit planches pour trois jeux,
/// chacune gardant sa copie de ce que sa voisine venait de lire. La tache d'ombre était
/// déjà partagée par toute la population, pour la même raison et une silhouette plus
/// tôt.
struct Sheets {
    /// Sa planche de repos.
    idle: Arc<Texture>,
    /// Celle de sa marche.
    walk: Arc<Texture>,
    /// Celle de sa mort.
    dead: Arc<Texture>,
}

impl Sheets {
    /// Décode les trois planches d'une silhouette.
    ///
    /// # Erreurs
    ///
    /// Si l'une ne se décode pas — elles sont intégrées au binaire, donc jamais en
    /// pratique.
    fn load(figure: &Figure) -> Result<Self, Error> {
        Ok(Self {
            idle: Arc::new(load_png_masked(figure.idle)?),
            walk: Arc::new(load_png_masked(figure.walk)?),
            dead: Arc::new(load_png_masked(figure.dead)?),
        })
    }
}

impl Monster {
    /// Pose une créature de cette silhouette debout sur cette case, cap et résistance
    /// tirés.
    ///
    /// **Le cap se tire, là où il venait du rang.** Le réparti sur le tour existait
    /// pour montrer trois vues dès la première image, trois créatures naissant côte à
    /// côte près de l'entrée ; semées à travers le labyrinthe, elles ne se voient plus
    /// ensemble, et un rang divisé par trois ne donnerait que trois caps pour seize
    /// créatures.
    ///
    /// **Les planches viennent décodées**, puisqu'elles sont celles de sa silhouette et
    /// non les siennes : c'est ce qui fait qu'elle ne peut plus échouer à naître.
    fn new(
        figure: &'static Figure,
        sheets: &Sheets,
        shadow: Arc<Texture>,
        rng: &mut Rng,
        grid: &Grid,
        map: &World,
        at: (u32, u32, u32),
    ) -> Self {
        // Le tour entier, au millième de tour : la vue ne distingue que huit secteurs,
        // donc une résolution plus fine ne se verrait pas, et une plus grossière
        // alignerait les caps sur les secteurs eux-mêmes.
        let facing = rng.below(1000) as f32 * core::f32::consts::TAU / 1000.0;
        // Le décalage porte sur `2 × SPREAD + 1` valeurs centrées sur la base, et la
        // base vaut au moins deux : la soustraction ne peut donc pas tomber à zéro,
        // ce qui ferait naître une créature déjà morte.
        let life = figure.life + rng.below(2 * SPREAD + 1) - SPREAD;

        // **Elle naît en visant sa propre case**, et le premier pas la renvoie aussitôt
        // choisir : `remaining` y vaut moins que le seuil d'arrivée, donc la patrouille
        // démarre d'elle-même à la première image. Lui choisir une sortie ici
        // demanderait de recopier ce que `reroute` fait déjà, et le côté d'entrée
        // n'aurait aucune valeur juste à donner — elle n'est entrée par nulle part.
        //
        // **Le côté d'entrée part vers le bas**, qui est exclu du choix comme toute
        // cage : il n'écarte donc aucune sortie horizontale, et le premier cap reste
        // celui que le tirage a donné.
        Self {
            figure,
            idle: Arc::clone(&sheets.idle),
            walk: Arc::clone(&sheets.walk),
            dead: Arc::clone(&sheets.dead),
            shadow,
            body: Body::stand(HALF, grid, map, at),
            facing,
            motion: Motion::Idle,
            phase: 0.0,
            hindered: 0,
            goal: at,
            entered: Side::Down,
            hunt: None,
            recoil: None,
            life,
        }
    }

    /// Le pas que son cap lui demande pour cette image, avant tout obstacle.
    ///
    /// **Il presse en poursuivant**, et c'est le seul endroit où la vitesse change :
    /// le reste de la marche ne sait pas qu'une course a lieu.
    fn step(&self, dt: f32) -> Vec3 {
        let pace = match self.hunt {
            Some(_) => SPEED * CHASE,
            None => SPEED,
        };
        Vec3::new(self.facing.cos(), self.facing.sin(), 0.0) * (pace * dt)
    }

    /// Suit les passages de son étage, d'une case à la suivante.
    ///
    /// **Le déplacement passe par le corps**, avec son propre gabarit : la glissade,
    /// la chute et le franchissement sont ceux du joueur, et c'est ce que l'extraction
    /// du corps a acheté. Une créature descend donc une rampe sans qu'une ligne d'ici
    /// le sache — mais elle ne l'emprunte jamais de son plein gré, les cages étant
    /// exclues du choix par [`onward`].
    ///
    /// **Elle ne se détourne plus de ce qui l'arrête, elle va quelque part.** C'est tout
    /// ce qui sépare cette marche de l'errance qui la précédait : le cap pointait vers
    /// une direction libre et un seuil le faisait pivoter quand le décor y mettait un
    /// mur, là où il pointe désormais vers un point de passage que le labyrinthe a
    /// ouvert. Le mur n'est plus sur le chemin, donc rien n'a plus à l'en détourner.
    ///
    /// **Rien ne l'arrête non plus** : ni le décor, qu'elle longe, ni une autre
    /// créature, qu'elle traverse. C'est la frontière du moteur appliquée telle quelle —
    /// seule la géométrie de cellule arrête, et un volume mobile n'en est pas —, et
    /// c'est ce que le jeu fera aussi des caisses et des ramassages posés au sol. Une
    /// exception pour les démons entre eux coûtait des blocages symétriques sans rien
    /// acheter de cohérent.
    ///
    /// **Ce qui reste de l'ancienne patience** garde un cas et un seul : une créature
    /// qui n'approche plus de son but pendant un cinquième de seconde repart vers la
    /// case qu'elle vient de quitter. Rien du décor ne devrait le déclencher, et c'est
    /// une garde plutôt qu'une politique.
    fn walk(&mut self, grid: &Grid, map: &World, prey: Option<Vec3>, dt: f32) {
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

        // Ce qu'elle était en train de faire, lu avant que la mémoire ne bouge : c'est
        // la bascule de la poursuite vers la patrouille qui demande un geste, pas l'état
        // lui-même.
        let chased = self.hunt.is_some();

        // **Le cap se reprend du but à chaque image**, et c'est ce qui règle la marche
        // en crabe : il pointait vers une direction libre, donc une glissade le laissait
        // diverger du déplacement réel et la silhouette avançait de travers. Visant un
        // point, il revient de lui-même sur ce qui est parcouru — et l'argument qui
        // écartait cette correction, « on longerait les murs indéfiniment », tombe avec
        // l'errance qui le portait.
        // **Voir le joueur remplace le point de passage, et rien d'autre** : le cap
        // pointe sur lui au lieu du centre de la case visée, le reste de la marche ne
        // bouge pas.
        //
        // **Le perdre de vue n'éteint pas la poursuite tout de suite** : elle va vers
        // l'endroit où il était, le temps que la mémoire dure. C'est ce qui fait qu'on
        // ne se décroche pas en passant un angle, sans qu'aucun chemin soit à calculer
        // — elle vise un point, pas quelqu'un.
        self.hunt = match prey.filter(|eye| self.sees(map, *eye)) {
            Some(eye) => Some((eye, MEMORY)),
            None => self
                .hunt
                .map(|(mark, left)| (mark, left - dt))
                .filter(|(_, left)| *left > 0.0),
        };

        self.facing = match self.hunt {
            Some((mark, _)) => bearing_to(self.body.centre(), mark, self.facing),
            None => {
                // **La patrouille se reprend une fois, au pas où la mémoire s'épuise** :
                // la course a mené la créature ailleurs, donc son ancien but n'est plus
                // voisin et le cap vers lui traverserait des murs.
                if chased {
                    self.reroute_here(grid);
                }
                self.bearing(grid)
            }
        };

        let step = self.step(dt);
        // **Le pas demandé reste celui du cap, même annulé**, et c'est ce qui fait
        // tomber le critère du bon côté : comparé à zéro, un pas nul paraîtrait
        // franchi en entier, la patience se remettrait à zéro, et deux créatures se
        // pousseraient indéfiniment sans jamais se détourner.
        let before = self.body.centre();
        let approach = self.remaining(grid);
        self.body.advance(map, step, dt);
        let after = self.body.centre();

        // **Ce qui se mesure est l'approche du but, non la distance franchie.** Les deux
        // divergent dès qu'on glisse le long d'une paroi en virant : le pas passe
        // entièrement et pourtant on ne gagne rien sur le but. C'est cette seconde chose
        // qui doit faire rechoisir.
        let left = self.remaining(grid);
        self.hindered = match left < approach {
            true => 0,
            false => self.hindered + 1,
        };

        // **Atteindre son but et renoncer sont deux gestes distincts**, et c'est la
        // position qui les sépare : arrivée, la créature est sur la case et choisit la
        // suivante depuis elle ; bloquée, elle est encore en chemin, donc la seule case
        // dont elle soit sûre est celle qu'elle vient de quitter.
        if left <= ARRIVED {
            self.reroute(grid);
        } else if self.hindered >= PATIENCE {
            self.turn_back(grid);
        }

        let gone = Vec3::new(after.x - before.x, after.y - before.y, 0.0);
        self.advance(gone.dot(gone).sqrt(), dt);
    }

    /// Choisit la case suivante depuis celle qu'elle vient d'atteindre.
    ///
    /// **Sans effet quand la case n'offre aucune sortie** : la créature garde alors son
    /// but, donc elle reste où elle est. Un labyrinthe engendré n'en produit pas, et une
    /// créature immobile vaut mieux qu'une panique dans un rappel de jeu.
    fn reroute(&mut self, grid: &Grid) {
        if let Some((next, side)) = onward(grid, self.goal, self.entered, self.facing) {
            self.goal = next;
            self.entered = side;
        }
        self.hindered = 0;
    }

    /// Repart vers la case qu'elle vient de quitter, sa route étant barrée.
    ///
    /// **C'est le seul demi-tour de la patrouille**, et il ne se déclenche que sur une
    /// gêne qui dure : une créature arrêtée par une autre dans un couloir ne peut ni
    /// passer ni contourner, donc revenir est la seule issue. En arrivant, le côté
    /// d'entrée exclura la case où elle était bloquée, et elle prendra une autre sortie.
    ///
    /// **Le voisin se relit de la grille plutôt que de se garder** : le côté d'entrée le
    /// désigne déjà, et garder la case précédente serait un troisième état à tenir
    /// d'accord avec les deux autres.
    fn turn_back(&mut self, grid: &Grid) {
        if let Some(back) = grid.neighbour(self.goal, self.entered) {
            self.goal = back;
            self.entered = self.entered.facing();
        }
        self.hindered = 0;
    }

    /// Vrai si elle voit le joueur d'ici : à portée, et sans décor entre les deux.
    ///
    /// **La distance d'abord, le rayon ensuite**, et l'ordre compte : un balayage coûte
    /// une traversée de cellules, une comparaison de carrés ne coûte rien, et la plupart
    /// des créatures d'un labyrinthe sont hors de portée à chaque image.
    ///
    /// **Le rayon part de l'œil de la créature et non de son centre** : au centre, il
    /// rase le sol sur une pente et se fait arrêter par lui. La cote est celle de sa
    /// demi-hauteur, qui est l'endroit d'où elle regarde.
    ///
    /// **Le critère est la fraction, jamais l'identifiant de surface** : celui-ci vaut
    /// zéro pour trois cas dont deux arrêtent — un portail non apparié, qui est un mur
    /// comme un autre, et un trajet tronqué. C'est la clause que le tir tient déjà, et
    /// la vue n'a aucune raison de la lire autrement.
    ///
    /// **Tout ce qui n'est pas une vue franche vaut « ne voit pas »** : un trajet
    /// tronqué par le budget du balayage, et un refus de cellule — qui arrive sur un
    /// bord de portail, à l'image où le corps en change. Prendre l'un des deux pour une
    /// vue dégagée ferait détecter à travers un mur une fois de temps en temps, ce qui
    /// est le genre de défaut qu'on cherche des heures.
    fn sees(&self, map: &World, eye: Vec3) -> bool {
        let here = self.body.centre();
        if !within_sight(here, eye) {
            return false;
        }

        let from = Vec3::new(here.x, here.y, here.z + HALF.z);
        match map.pick_checked(self.body.cell(), from, eye, Surfaces::Solid) {
            Ok(Sweep::Reached(hit)) => hit.fraction >= 1.0 && !hit.incomplete,
            _ => false,
        }
    }

    /// L'écart horizontal qui la sépare du centre de la case visée.
    ///
    /// **Le point se reprend de la grille plutôt que de se garder**, et c'est un choix :
    /// une cote cachée à côté de la case serait une seconde source de vérité, qui
    /// divergerait le jour où l'une des deux bougerait seule. Le calcul est une
    /// recherche dans les six volées et deux multiplications.
    ///
    /// **Horizontal, parce que la cote verticale n'est pas la sienne** : le point visé
    /// est un sol, la créature se tient dessus, et compter l'écart vertical dirait
    /// qu'elle n'arrive jamais.
    /// **Le décalage se prend du côté d'entrée et non du cap courant** : pris de
    /// celui-ci, il tournerait avec lui à chaque correction et la trajectoire
    /// serpenterait au lieu de longer.
    ///
    /// Le sens de marche est l'opposé du côté par lequel on entre — on entre par
    /// l'ouest en allant vers l'est —, et la droite d'un vecteur `(x, y)` est
    /// `(y, −x)`.
    fn offset(&self, grid: &Grid) -> (f32, f32) {
        let here = self.body.centre();
        let there = export::ground(grid, self.goal);

        let (dx, dy, _) = self.entered.facing().step();
        let aside = (dy as f32 * DRIFT, -(dx as f32) * DRIFT);

        (there[0] + aside.0 - here.x, there[1] + aside.1 - here.y)
    }

    /// Le cap qui mène au centre de la case visée, en radians.
    fn bearing(&self, grid: &Grid) -> f32 {
        let (dx, dy) = self.offset(grid);
        bearing_to(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(dx, dy, 0.0),
            self.facing,
        )
    }

    /// Reprend la patrouille depuis la case où elle se trouve.
    ///
    /// **Une poursuite abandonne le circuit**, et c'est pourquoi ceci existe : la
    /// créature a suivi le joueur en ligne droite, donc la case qu'elle visait avant
    /// n'est plus voisine de la sienne et le cap vers elle traverserait des murs. Elle
    /// repart donc de là où elle est.
    ///
    /// **Hors de la grille, elle garde son but** : la case calculée peut tomber dehors
    /// si une poursuite l'a menée aux bords, et la grille le dirait en refusant. Garder
    /// l'ancien but la ramène vers le labyrinthe, ce qui est la seule réponse utile.
    fn reroute_here(&mut self, grid: &Grid) {
        let here = self.body.centre();
        let (x, y, z) = export::cell_at([here.x, here.y, here.z]);
        let (width, height, levels) = grid.extent();
        let inside = (0..width as i32).contains(&x)
            && (0..height as i32).contains(&y)
            && (0..levels as i32).contains(&z);

        if inside {
            self.goal = (x as u32, y as u32, z as u32);
            // Le côté d'entrée n'a plus de sens après une course libre : la cage du bas
            // est exclue du choix de toute façon, donc elle n'écarte aucune sortie.
            self.entered = Side::Down;
        }
        self.reroute(grid);
    }

    /// Ce qui lui reste à parcourir jusqu'au centre de la case visée.
    fn remaining(&self, grid: &Grid) -> f32 {
        let (dx, dy) = self.offset(grid);
        (dx * dx + dy * dy).sqrt()
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

    /// Les demi-étendues que le tir doit tester contre elle.
    ///
    /// **De la silhouette et non du module**, là où le volume de marche est commun :
    /// voir [`Figure::touch`], qui porte la mesure et ce qu'elle a corrigé.
    pub fn hittable(&self) -> Vec3 {
        self.figure.touch()
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

/// La case voisine vers laquelle poursuivre, et le côté par lequel on y entrera.
///
/// **Le plus proche du cap, et c'est ce qui fait une patrouille plutôt qu'une
/// errance** : la créature va tout droit tant que le couloir le permet, prend le
/// moindre écart quand il tourne, et fait demi-tour au fond d'un cul-de-sac. Aucun
/// tirage n'intervient — une direction au hasard demanderait une graine par créature,
/// et ferait perdre ce que la graine du labyrinthe achète, un défaut qui se rejoue.
///
/// **Les cages sont exclues**, et c'est ce qui tient l'étage : [`Side::is_vertical`] les
/// nomme, donc la clause se lit au lieu de se déduire d'une cote. Une créature reste
/// ainsi sur le niveau où elle est née, ce qui est la seule façon de savoir où la
/// chercher — et le relevé de titre dit le dénivelé pour la même raison.
///
/// **Le côté d'entrée s'exclut tant qu'autre chose existe** : sans cela, « le plus
/// proche du cap » renverrait la créature d'où elle vient dès qu'un couloir tourne de
/// plus d'un quart, et deux cases se partageraient toute sa patrouille.
///
/// **Rien ne rend `None` sur un labyrinthe engendré** : le générateur n'y laisse aucune
/// case isolée. Le cas est traité parce qu'un décor écrit à la main pourrait en porter
/// une, et qu'une créature immobile vaut mieux qu'une panique dans un rappel de jeu.
fn onward(
    grid: &Grid,
    at: (u32, u32, u32),
    entered: Side,
    facing: f32,
) -> Option<((u32, u32, u32), Side)> {
    let open = |side: Side| {
        (!side.is_vertical() && !grid.has_wall(at, side))
            .then(|| grid.neighbour(at, side))
            .flatten()
            .map(|next| (next, side))
    };

    // Le demi-tour en dernier recours : on l'écarte du choix, et on n'y revient que si
    // la case n'offre rien d'autre.
    Side::ALL
        .into_iter()
        .filter(|side| *side != entered)
        .filter_map(open)
        .min_by(|(_, here), (_, there)| {
            deviation(facing, *here).total_cmp(&deviation(facing, *there))
        })
        .or_else(|| open(entered))
        .map(|(next, side)| (next, side.facing()))
}

/// Vrai si ces deux points sont à portée de vue l'un de l'autre.
///
/// **À part du rayon pour être éprouvée** : la portée est une fonction d'entrées vers
/// des sorties, là où la vue dégagée demande un décor — et un décor d'épreuve finit
/// toujours par mettre un mur là où l'on voulait mesurer une distance. Les deux clauses
/// se gardent donc séparément.
///
/// **Les carrés se comparent sans racine**, ce qui est demandé étant un ordre.
fn within_sight(here: Vec3, eye: Vec3) -> bool {
    let gap = eye - here;
    gap.dot(gap) <= SIGHT * SIGHT
}

/// Le cap qui mène d'un point à un autre, en radians.
///
/// **Rend le cap de repli quand les deux points se confondent**, ce qui arrive au pas où
/// un but vient d'être atteint : un `atan2` de deux zéros rendrait zéro, donc la
/// créature se tournerait vers l'est pour une image avant de reprendre son chemin, et
/// cela se verrait.
///
/// **La cote verticale ne compte pas** : une créature marche à plat, et viser un œil
/// plus haut que soi ne doit pas incliner le pas.
fn bearing_to(from: Vec3, to: Vec3, fallback: f32) -> f32 {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    match dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
        true => fallback,
        false => dy.atan2(dx),
    }
}

/// De combien un côté s'écarte d'un cap, en radians, entre zéro et un demi-tour.
///
/// **Replié sur un demi-tour, parce que c'est un écart et non une rotation** : sans ce
/// repli, un cap juste au-dessus du tour complet paraîtrait aux antipodes du côté qu'il
/// désigne, et le choix partirait à l'opposé une fois sur deux.
fn deviation(facing: f32, side: Side) -> f32 {
    let (dx, dy, _) = side.step();
    let gap = (dy as f32).atan2(dx as f32) - facing;
    let turn = core::f32::consts::TAU;
    let folded = gap.rem_euclid(turn);
    folded.min(turn - folded)
}

/// Le nombre de créatures qu'un labyrinthe porte.
///
/// **Seize pour cinq cent douze cases, réglé à l'écran**, soit une tous les trente-deux
/// pas environ : assez pour qu'une traversée en croise, trop peu pour que le labyrinthe
/// devienne une foule. Trois — une par silhouette, ce que le compte valait tant qu'il
/// était celui de la table — donnait une traversée où l'on ne croisait rien.
///
/// **Il ne tient plus au nombre de silhouettes**, et c'est tout l'objet : la table dit
/// à quoi une créature ressemble, pas combien il y en a.
pub const POPULATION: usize = 16;

/// Peuple le labyrinthe aux cases que le semis a tirées.
///
/// **Les cases viennent du dehors et ne se cherchent plus ici**, parce qu'un seul
/// semis sert toutes les familles : c'est ce qui garantit qu'aucun ramassage ne
/// naîtra dans une créature, et le garantir demande un tirage unique.
///
/// **Une créature par case reçue, et c'est le refus** : moins de cases que de créatures
/// demandées en rend moins, là où un repli sur la case de départ les faisait naître
/// dans le joueur dès que le compte montait.
///
/// **La silhouette se prend à tour de rôle et ne se tire pas.** Un tirage peut rendre
/// seize fois la même par malchance, ce qui ruine ce que ce jeu démontre — trois
/// silhouettes qui ne se confondent pas de loin. Le tour de rôle les répartit également
/// par construction, et la dispersion est déjà dans les cases, que le semis a brassées.
///
/// **La tache d'ombre et les planches se décodent ici, une fois pour toutes** : une
/// tache pour la population, trois jeux de planches pour les trois silhouettes, et le
/// partage par `Arc` les rend sans coût quel que soit le compte.
///
/// # Erreurs
///
/// Si une planche ne se décode pas — elles sont intégrées au binaire, donc jamais en
/// pratique.
pub fn population(
    spots: &[(u32, u32, u32)],
    rng: &mut Rng,
    grid: &Grid,
    map: &World,
) -> Result<Vec<Monster>, Error> {
    let shadow = Arc::new(shadow_texture());
    let sheets = FIGURES
        .iter()
        .map(Sheets::load)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(spots
        .iter()
        .enumerate()
        .map(|(rank, &at)| {
            let kind = rank % FIGURES.len();
            Monster::new(
                &FIGURES[kind],
                &sheets[kind],
                Arc::clone(&shadow),
                rng,
                grid,
                map,
                at,
            )
        })
        .collect())
}

/// Fait patrouiller la population, chacune sur son étage.
///
/// **Une créature n'en arrête plus une autre**, et c'est un retour à la frontière du
/// moteur plutôt qu'un renoncement : seule la géométrie de cellule arrête, un démon n'a
/// ni portail ni adjacence, et le jeu ne testera pas davantage une caisse ou un
/// ramassage posé au sol. Les traiter à part était une exception, et elle coûtait cher.
///
/// **Ce qu'elle coûtait, mesuré** : deux créatures qui se croisent visent le même point
/// de passage, donc elles convergent ; l'évitement étant symétrique, chacune annulait le
/// pas de l'autre et aucune ne cédait. Relevé sur la graine `0x1`, deux silhouettes
/// immobiles trois secondes durant, qu'un demi-tour renvoyait converger dans l'autre
/// sens. Un décalage par créature n'y suffisait pas non plus, deux sens opposés pouvant
/// se ranger du même côté.
///
/// **Et la poursuite l'aurait de toute façon défait** : plusieurs créatures venant vers
/// le joueur convergent par construction, donc l'évitement y produirait des démons qui
/// s'éloignent au moment où ils devraient arriver.
///
/// **Ce qui reste du croisement est visuel** : elles serrent à droite, donc deux
/// trajectoires opposées se frôlent au lieu de se superposer. Voir [`DRIFT`].
/// **Elles viennent vers le joueur dès qu'elles le voient**, et reprennent leur circuit
/// dès qu'elles le perdent. L'œil est passé en option parce que la mort le retire : une
/// partie finie fige la traversée, et une créature n'a alors plus personne à suivre.
pub fn stroll(monsters: &mut [Monster], grid: &Grid, map: &World, prey: Option<Vec3>, dt: f32) {
    for monster in monsters {
        monster.walk(grid, map, prey, dt);
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
) -> Result<(), CoreError> {
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
