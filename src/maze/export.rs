// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La grille écrite en cellules et portails, telle que le moteur la charge.
//!
//! Une case devient une cellule — un volume fermé : son empreinte au sol, son
//! plafond, et un mur par côté. Un mur percé devient un **portail** à la place de
//! sa surface, et les deux cellules qu'il sépare en écrivent chacune un, aux
//! mêmes positions et en sens inverse.
//!
//! **Rien ici ne vérifie la cohérence géométrique à notre place.** Le chargement
//! accepte sans erreur un décor faux — une face retournée, une cellule mal
//! fermée, deux portails qui ne se rejoignent pas —, et le défaut se voit à
//! l'image, ou ne se voit qu'en mouvement. Les assertions de ce module sont donc
//! le seul filet, et chacune répond à un défaut que l'autre dépôt a vécu.
//!
//! **Les passages d'étage sont ignorés.** Les niveaux sortent en composantes
//! séparées, chacune close : un portail non apparié est un mur et non une erreur,
//! et deux cellules superposées sans lien sont un cas que le moteur éprouve déjà.
//! La cellule-escalier les reliera.

use super::grid::{Grid, Shape, Side, Stair};

#[cfg(test)]
mod tests;

/// Le côté d'une case, en unités de monde — une unité vaut un mètre.
pub const CELL: f32 = 4.0;

/// La hauteur d'un étage, du sol d'un niveau au sol du suivant.
///
/// Trois mètres, et c'est l'escalier qui l'impose : douze marches de 0,25 y
/// montent exactement. Dix marches de 0,3 n'y arrivent pas — la cote accumulée
/// retombe à deux ulp du sol de l'étage, et le portail du haut cesse de
/// s'apparier, sans erreur, comme toujours avec l'appariement.
///
/// Elle valait le côté d'une case quand on visait une rampe à 45°. Ce n'était pas
/// une contrainte du format : l'axe de lightmap prend la direction **unitaire**,
/// dont le carré vaut un quelle que soit la pente.
pub const LEVEL: f32 = 3.0;

/// La hauteur sous plafond, ce qui laisse une dalle d'un demi-mètre entre deux
/// étages.
///
/// **Deux mètres et demi parce que c'est exact en binaire**, et 2,4 ne l'est pas.
/// La somme de Newell qui donne la normale d'une surface se fait en simple
/// précision : sur un prisme à huit sommets, le résidu d'une cote inexacte reste
/// sous la tolérance du chargement ; sur le flanc d'une cage, qui en a
/// vingt-huit, il la dépasse — et le repère de la surface est refusé pour sortir
/// d'un plan dont il ne sort pas. Le défaut était là depuis le premier export,
/// et seule une cellule assez grande le réveille.
pub const CEILING: f32 = 2.5;

/// La demi-épaisseur d'un mur entre deux cellules.
///
/// **C'est elle qui donne une épaisseur aux murs**, et rien d'autre ne le peut :
/// une cellule occupe `CELL − 2 × MARGIN` au lieu de sa case entière, et la
/// marge laissée de chaque côté est du solide. Deux cellules jointives ne
/// laissaient aucune place à un mur — il y était une surface sans épaisseur, et
/// un couloir ressemblait à une boîte de papier.
const MARGIN: f32 = 0.5;

/// Les texels par unité de monde.
///
/// Les planches font 512 de côté, donc une couvre exactement une case. La
/// coordonnée la plus lointaine d'un labyrinthe de seize cases vaut alors 8192,
/// soit la moitié de ce que le chargement accepte.
const TEXELS: f32 = 128.0;

/// Les luxels par unité de monde.
///
/// Quatre, soit un luxel tous les vingt-cinq centimètres. Un seul par unité —
/// ce que les décors du moteur prennent — donnerait quatre luxels en travers
/// d'un mur, trop grossier pour l'éclairage que l'étape 8 veut y poser. Le carré
/// de l'axe vaut donc 16, qui est bien une puissance de deux.
const LUXELS: f32 = 4.0;

/// Le côté intérieur d'une cellule de case, sur un axe horizontal.
///
/// Publiée parce qu'elle borne ce qui peut circuler : un volume plus large qu'une
/// cellule coince dans un couloir, et la cause se chercherait dans le
/// déplacement.
pub const INNER: f32 = CELL - 2.0 * MARGIN;

/// Le nombre de marches d'une volée.
const STEPS: u32 = 12;

/// La hauteur d'une marche.
///
/// Elle vaut aussi le seuil que l'étape 2 donnera au déplacement : ce qu'on monte
/// sans sauter se mesure ici, et nulle part ailleurs.
const RISE: f32 = LEVEL / STEPS as f32;

/// Le palier au pied de la volée, entre le portail d'entrée et la première
/// contremarche.
///
/// **La géométrie l'impose, pas le confort.** Le portail d'entrée doit border le
/// volume sur toute sa hauteur pour s'apparier avec le passage ; une contremarche
/// qui partirait de l'entrée même tomberait dans le plan du portail, qui
/// deviendrait dégénéré.
const LANDING: f32 = 0.375;

/// La profondeur d'une marche.
///
/// Ce qui reste de la cellule une fois le palier pris, divisé par le nombre de
/// marches : `7/32`, donc exact en binaire comme toutes les cotes qui en
/// découlent. La pente vaut `RISE / TREAD`, soit `8/7` — raide, et c'est le prix
/// du palier.
const TREAD: f32 = (INNER - LANDING) / STEPS as f32;

/// Ce que le sol peut monter par unité parcourue à l'horizontale.
///
/// **Le maximum des deux formes de cage**, et c'est l'escalier qui le donne :
/// `RISE / TREAD`, soit `8/7`, contre `1` pour une rampe. Ailleurs le sol est
/// plat.
///
/// **C'est ce qu'il faut à qui pose un volume** : une empreinte de soixante
/// centimètres dépasse le palier d'une cage, qui en fait trente-sept, et
/// surplombe donc des marches. Posée à la seule cote du sol, elle les pénètre et
/// part dans le solide. Le dégagement vaut la demi-étendue horizontale fois cette
/// pente, et il ne se devine pas depuis le jeu : seul l'export sait ce que son
/// sol fait.
pub const SLOPE: f32 = RISE / TREAD;

/// L'emplacement du matériau des murs.
const WALL: u32 = 1;

/// Celui du sol, du plafond et des marches — girons et contremarches.
const FLOOR: u32 = 2;

/// Les surfaces qu'une cellule de case ou de passage réserve.
///
/// **Réservées et non employées** : un prisme dont un côté s'ouvre en porte
/// moins, et l'emplacement libre reste libre. C'est ce qui rend l'identifiant
/// d'une surface fonction de la seule position, donc stable d'une graine à
/// l'autre — et l'empreinte du cache de lightmaps les hache.
const PRISM_SURFACES: u32 = 6;

/// Ses portails, pour la même raison.
const PRISM_PORTALS: u32 = 4;

/// De combien une lampe pend sous son plafond.
///
/// **Pas collée, et le calcul l'impose** : l'atténuation porte un terme de
/// Lambert, donc une lampe posée dans le plan du plafond l'aurait dans son dos et
/// le laisserait noir.
const DROP: f32 = 0.3;

/// Le rayon d'une lampe de case.
///
/// De quoi couvrir sa cellule jusqu'aux coins et déborder sur ses voisines à un
/// portail, qui sont tout ce qu'elle peut atteindre.
const ROOM: f32 = 5.0;

/// Celui d'une lampe de cage, qui a deux étages à remplir.
const CAGE: f32 = 8.0;

/// La teinte des lampes, et son octet réservé.
///
/// Blanche et provisoire : l'ambiance se règle à la cuisson, et un labyrinthe où
/// des démons passent la voudra plus froide. Le quatrième octet est **réservé et
/// nul obligatoire**, ce n'est pas du remplissage.
const GLOW: [u8; 4] = [0xFF, 0xF4, 0xE0, 0x00];

/// Les surfaces qu'une cage réserve : deux par marche, plus le palier, le
/// plafond, les deux flancs et le linteau de son entrée.
///
/// **Réservées par les deux formes de cage**, et une cage en rampe n'en emploie
/// que cinq. C'est la même clause que pour un prisme dont un côté s'ouvre :
/// l'identifiant d'une surface reste fonction de la seule position, donc stable
/// quand la graine change la forme tirée — et l'empreinte du cache de lightmaps
/// les hache.
const STAIR_SURFACES: u32 = 2 * STEPS + 5;

/// Celles qu'une cage en rampe écrit : la rampe, le plafond, les deux flancs et
/// le linteau.
const RAMP_SURFACES: u32 = 5;

/// Ses portails : l'entrée et la sortie, et jamais plus. Les deux autres faces
/// de l'axe de montée n'ont pas de volume derrière elles.
const STAIR_PORTALS: u32 = 2;

/// Les quatre côtés horizontaux, dans l'ordre des arêtes de l'empreinte.
const EDGES: [Side; 4] = [Side::South, Side::East, Side::North, Side::West];

/// La direction unitaire de chaque arête, dans le même ordre.
///
/// Elle sert d'axe horizontal aux deux repères, à l'échelle près. **Unitaire et
/// non l'arête elle-même** : son carré vaut alors un, qui est une puissance de
/// deux quelle que soit la longueur du mur, et la taille d'une case cesse d'être
/// contrainte. Les décors du moteur ne s'accordent pas sur ce point — l'un prend
/// la direction, les trois autres l'arête, et c'est ce second choix qui leur
/// impose des longueurs en puissance de deux.
const ALONG: [[f32; 3]; 4] = [
    [1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, -1.0, 0.0],
];

/// L'axe vertical des murs.
const UPWARD: [f32; 3] = [0.0, 0.0, 1.0];

/// Les deux axes du sol et du plafond.
const FLAT: ([f32; 3], [f32; 3]) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);

/// La signature des trois genres de fichier du moteur.
const SIGNATURE: [u8; 4] = [b'S', b'C', b'G', 0x1A];

/// La version du format des cartes.
const VERSION: u32 = 1;

/// La longueur de l'en-tête commun.
const HEADER: usize = 20;

/// Celle d'une entrée de la table des sections.
const ENTRY: usize = 12;

/// Écrit la carte que `World::load` charge.
///
/// Les cellules sortent dans l'ordre des axes, et cet ordre **est** une donnée :
/// quand deux cellules superposées contiennent le même point, c'est la première
/// du fichier que la localisation rend.
///
/// # Panique
///
/// Si une empreinte tourne à l'envers, si un axe de lightmap n'a pas un carré en
/// puissance de deux, ou si une coordonnée de texture dépasse ce que le
/// chargement accepte. Les trois sont des défauts de ce module, jamais des
/// entrées de l'appelant.
pub fn world(grid: &Grid) -> Vec<u8> {
    let sections = [
        (b"CELL", cells(grid)),
        (b"ENTS", entities(grid)),
        (b"LGTS", lights(grid)),
        (b"MATS", materials()),
    ];

    // Les genres se rangent par ordre croissant et pavent le fichier sans trou :
    // c'est l'ordre alphabétique de leurs quatre lettres, et la première section
    // commence juste après la table.
    let first = HEADER + ENTRY * sections.len();
    let total = first + sections.iter().map(|(_, body)| body.len()).sum::<usize>();

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&SIGNATURE);
    out.extend_from_slice(b"WRLD");
    words(&[VERSION, total as u32, sections.len() as u32], &mut out);

    let mut offset = first;
    for (tag, body) in &sections {
        out.extend_from_slice(*tag);
        words(&[offset as u32, body.len() as u32], &mut out);
        offset += body.len();
    }

    for (_, body) in &sections {
        out.extend_from_slice(body);
    }
    out
}

/// L'identifiant que la carte donne à une case.
///
/// Fonction de la seule position, et non d'un compteur qui suivrait l'ordre du
/// creusement : c'est ce qui rend l'empreinte du cache de lightmaps réutilisable
/// d'une génération à l'autre, puisqu'elle hache les identifiants.
pub fn cell_id(grid: &Grid, at: (u32, u32, u32)) -> u32 {
    let (width, height, _) = grid.extent();
    at.2 * height * width + at.1 * width + at.0 + 1
}

/// L'identifiant de la cellule de passage qui prolonge une case vers l'est ou
/// vers le nord.
///
/// Un passage appartient à la case de plus petit rang, et à l'un de ses deux
/// côtés croissants : c'est ce qui lui donne un nom unique sans compteur, et
/// donc un identifiant stable d'une génération à l'autre.
fn gate_id(grid: &Grid, at: (u32, u32, u32), side: Side) -> u32 {
    let (width, height, levels) = grid.extent();
    let cells = width * height * levels;
    let lane = u32::from(side == Side::North);
    cells + (cell_id(grid, at) - 1) * 2 + lane + 1
}

/// La section des cellules : une par case, une par cage, et une par passage.
///
/// **Les cases d'abord, les passages ensuite**, et cet ordre est une donnée :
/// quand deux cellules contiennent le même point, c'est la première du fichier
/// que la localisation rend. Les cases sont ce qu'on veut qu'elle rende.
///
/// Une cage prend la place des **deux** cases qu'elle occupe : elle sort au rang
/// de son pied, et la case du dessus ne sort pas du tout.
fn cells(grid: &Grid) -> Vec<u8> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::new();

    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                let at = (x, y, z);
                if let Some(flight) = flight_from(grid, at) {
                    stair(grid, flight, &mut out);
                } else if !under_flight(grid, at) {
                    cell(grid, at, &mut out);
                }
            }
        }
    }
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                for side in [Side::East, Side::North] {
                    if !grid.has_wall((x, y, z), side) {
                        gate(grid, (x, y, z), side, &mut out);
                    }
                }
            }
        }
    }
    out
}

/// La volée qui part de cette case, s'il y en a une.
fn flight_from(grid: &Grid, at: (u32, u32, u32)) -> Option<Stair> {
    grid.stairs().iter().copied().find(|stair| stair.foot == at)
}

/// Vrai si cette case est la tête d'une volée, donc déjà prise par sa cage.
fn under_flight(grid: &Grid, at: (u32, u32, u32)) -> bool {
    grid.stairs().iter().any(|stair| stair.head() == at)
}

/// La cellule qui contient une case.
///
/// Ce n'est plus toujours la sienne : une cage couvre deux cases superposées, et
/// les deux rendent l'identifiant de son pied.
pub fn cover(grid: &Grid, at: (u32, u32, u32)) -> u32 {
    let flight = flight_of(grid, at);
    cell_id(grid, flight.map_or(at, |stair| stair.foot))
}

/// Le sol de la cellule qui contient une case, en son point le plus sûr.
///
/// **Le centre ne convient pas pour une case d'escalier** : au milieu de la cage,
/// le sol est déjà monté à mi-étage, et une cote prise à l'étage de la case y
/// tombe dans le solide. Ce point-ci se place au milieu du palier pour la case du
/// bas, et sur la dernière marche pour celle du haut — les deux seuls endroits de
/// la cage dont la cote du sol est celle d'un étage.
///
/// **Une cage en rampe n'a aucun endroit plat**, donc aucune des deux cases n'y a
/// la cote de son étage : le point est alors le milieu de la rampe, à mi-hauteur,
/// et il est le même pour le pied et pour la tête puisque la cellule l'est.
///
/// Ce qu'il rend est le **sol**, et l'appelant y ajoute ce qu'il lui faut de
/// dégagement vertical — une hauteur d'œil, un mètre. Il ne prend donc pas de
/// demi-étendue : relever une boîte au-dessus d'une pente dépend de sa taille,
/// et c'est au seul qui en pose une de le calculer.
pub fn ground(grid: &Grid, at: (u32, u32, u32)) -> [f32; 3] {
    let centre = [(at.0 as f32 + 0.5) * CELL, (at.1 as f32 + 0.5) * CELL];
    let Some(stair) = flight_of(grid, at) else {
        return [centre[0], centre[1], floor_of(at.2)];
    };
    if stair.shape == Shape::Ramp {
        return [centre[0], centre[1], floor_of(stair.foot.2) + LEVEL / 2.0];
    }

    // Le milieu du palier, et la dernière marche est à la même distance du bord
    // opposé : un seul décalage sert les deux cases.
    let offset = INNER / 2.0 - LANDING / 2.0;
    let towards = if stair.foot == at {
        stair.climb.facing()
    } else {
        stair.climb
    };
    let (dx, dy, _) = towards.step();
    [
        centre[0] + dx as f32 * offset,
        centre[1] + dy as f32 * offset,
        floor_of(at.2),
    ]
}

/// La volée dont cette case est le pied ou la tête.
fn flight_of(grid: &Grid, at: (u32, u32, u32)) -> Option<Stair> {
    grid.stairs()
        .iter()
        .copied()
        .find(|stair| stair.foot == at || stair.head() == at)
}

/// La section des lumières : une par cellule de case, une par cage.
///
/// **Aucune dans les passages**, et c'est la clause de propagation qui le permet :
/// l'ensemble des occulteurs d'une cellule est elle-même et ses voisines à **un**
/// portail, donc un passage reçoit la lumière des deux cases qu'il relie. Une
/// cellule à deux portails de toute lampe resterait noire, et c'est ce qui donne
/// sa lampe à chaque cage.
///
/// **Rien n'est cuit ici.** Une soumission dont la cellule n'a pas d'atlas retombe
/// sur le chemin non éclairé, sans erreur : ces lumières attendent la cuisson, et
/// l'ambiance se règle avec elle. Les écrire maintenant évite de réexporter, parce
/// que c'est la **géométrie** qui décide de ce qu'une lampe peut atteindre.
fn lights(grid: &Grid) -> Vec<u8> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::new();

    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                let at = (x, y, z);
                if under_flight(grid, at) {
                    continue;
                }
                let tall = flight_from(grid, at).is_some();
                let spot = [
                    (x as f32 + 0.5) * CELL,
                    (y as f32 + 0.5) * CELL,
                    floor_of(at.2) + if tall { LEVEL + CEILING } else { CEILING } - DROP,
                ];
                light(
                    cell_id(grid, at),
                    spot,
                    if tall { CAGE } else { ROOM },
                    &mut out,
                );
            }
        }
    }
    out
}

/// Une lumière : son identifiant, sa position, son rayon, sa teinte.
fn light(id: u32, spot: [f32; 3], radius: f32, out: &mut Vec<u8>) {
    words(&[id], out);
    floats(&[spot[0], spot[1], spot[2], radius], out);
    out.extend_from_slice(&GLOW);
}

/// La section des entités : où l'on entre, et par où l'on sort.
///
/// **La carte les porte plutôt qu'une structure parallèle** : le moteur décode
/// cette section, la valide — la cellule doit exister, le quaternion ne doit pas
/// être nul — et l'expose sans jamais lire la classe ni les données. Le placement
/// vit donc avec le décor qu'il désigne, et se recharge avec lui.
fn entities(grid: &Grid) -> Vec<u8> {
    let mut out = Vec::new();
    for (rank, (class, at)) in [("start", grid.start()), ("exit", grid.exit())]
        .into_iter()
        .enumerate()
    {
        entity(
            rank as u32 + 1,
            cover(grid, at),
            class,
            ground(grid, at),
            &mut out,
        );
    }
    out
}

/// Une entité : sa longueur, ses identifiants, sa classe, sa pose, ses données.
fn entity(id: u32, cell: u32, class: &str, spot: [f32; 3], out: &mut Vec<u8>) {
    let mut body = Vec::new();
    words(&[id, cell], &mut body);
    body.extend_from_slice(&(class.len() as u16).to_le_bytes());
    body.extend_from_slice(class.as_bytes());
    // L'identité se range `x, y, z, w`, la partie réelle en dernier : un départ
    // n'a pas d'orientation, et un quaternion nul serait refusé.
    floats(&[spot[0], spot[1], spot[2], 0.0, 0.0, 0.0, 1.0], &mut body);
    // Aucune donnée : la classe suffit à les distinguer, et un bloc que le moteur
    // recopie sans le lire ne porterait rien que le jeu ne sache déjà.
    words(&[0], &mut body);

    words(&[body.len() as u32], out);
    out.extend_from_slice(&body);
}

/// La section des matériaux : le fichier porte des noms, jamais des images.
fn materials() -> Vec<u8> {
    let mut out = Vec::new();
    for (id, name) in [(WALL, "wall"), (FLOOR, "floor")] {
        words(&[id], &mut out);
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    out
}

/// La cellule d'une case : un carré plus petit que sa case, centré dedans.
///
/// **Plus petite, et c'est tout l'objet de ce découpage** : la marge laissée de
/// chaque côté devient du solide, si bien qu'un mur entre deux cases a une
/// épaisseur de deux marges. Des cellules jointives n'en laissaient aucune — un
/// mur y était une surface sans épaisseur, l'arête de deux murs était
/// infiniment fine, et un couloir se lisait comme une boîte de papier.
fn cell(grid: &Grid, at: (u32, u32, u32), out: &mut Vec<u8>) {
    let rank = cell_id(grid, at) - 1;
    let footprint = [
        [inner_low(at.0), inner_low(at.1)],
        [inner_high(at.0), inner_low(at.1)],
        [inner_high(at.0), inner_high(at.1)],
        [inner_low(at.0), inner_high(at.1)],
    ];
    let open = [
        !grid.has_wall(at, EDGES[0]),
        !grid.has_wall(at, EDGES[1]),
        !grid.has_wall(at, EDGES[2]),
        !grid.has_wall(at, EDGES[3]),
    ];
    prism(
        rank + 1,
        rank * PRISM_SURFACES,
        rank * PRISM_PORTALS,
        footprint,
        at.2,
        open,
        out,
    );
}

/// La cellule d'un passage : le couloir court qui perce un mur.
///
/// Elle occupe l'épaisseur du mur, à cheval sur la frontière des deux cases, et
/// porte un portail à chaque bout. C'est elle qui donne son embrasure au
/// passage, et elle seule.
fn gate(grid: &Grid, at: (u32, u32, u32), side: Side, out: &mut Vec<u8>) {
    let (width, height, levels) = grid.extent();
    let cells = width * height * levels;
    let rank = gate_id(grid, at, side) - 1;

    // Le passage est centré sur la frontière des deux cases, et ses deux bouts
    // sortent de `coord` comme les bornes des cellules qu'il relie : les mêmes
    // bits des deux côtés, ce que l'appariement exige sans tolérance.
    let (footprint, open) = match side {
        Side::East => (
            [
                [inner_high(at.0), inner_low(at.1)],
                [inner_low(at.0 + 1), inner_low(at.1)],
                [inner_low(at.0 + 1), inner_high(at.1)],
                [inner_high(at.0), inner_high(at.1)],
            ],
            [false, true, false, true],
        ),
        _ => (
            [
                [inner_low(at.0), inner_high(at.1)],
                [inner_high(at.0), inner_high(at.1)],
                [inner_high(at.0), inner_low(at.1 + 1)],
                [inner_low(at.0), inner_low(at.1 + 1)],
            ],
            [true, false, true, false],
        ),
    };

    // Les passages prennent leurs identifiants de surface et de portail après
    // ceux de toutes les cases, pour que ni les uns ni les autres ne dépendent
    // du nombre de passages — qui change avec la graine.
    let slot = rank - cells;
    prism(
        rank + 1,
        cells * PRISM_SURFACES + slot * PRISM_SURFACES,
        cells * PRISM_PORTALS + slot * PRISM_PORTALS,
        footprint,
        at.2,
        open,
        out,
    );
}

/// Le repère d'une cage d'escalier : ses quatre bords, et le sol de son étage.
///
/// **L'ordre des deux bords transverses n'est pas libre** : il est choisi pour
/// que le couple (montée, transverse) ait son produit vectoriel vers le haut,
/// quelle que soit la direction de la volée. Tout l'enroulement de la cage en
/// découle — un seul ordre d'indices sert alors les vingt-cinq surfaces du sol,
/// sans distinguer les quatre orientations.
struct Flight {
    /// La cote de la face d'entrée sur l'axe de montée.
    entry: f32,
    /// Celle de la face de sortie.
    exit: f32,
    /// Les deux bords de l'axe transverse, dans cet ordre.
    sides: (f32, f32),
    /// Vrai quand la montée suit l'axe des abscisses.
    lengthwise: bool,
    /// Le sol de l'étage du bas.
    base: f32,
}

impl Flight {
    /// Le repère de la volée qui part de cette case.
    fn new(at: (u32, u32, u32), climb: Side) -> Self {
        let (west, east) = (inner_low(at.0), inner_high(at.0));
        let (south, north) = (inner_low(at.1), inner_high(at.1));
        let base = floor_of(at.2);
        match climb {
            Side::East => Self {
                entry: west,
                exit: east,
                sides: (south, north),
                lengthwise: true,
                base,
            },
            Side::West => Self {
                entry: east,
                exit: west,
                sides: (north, south),
                lengthwise: true,
                base,
            },
            // Et l'inverse : la montée suit les ordonnées, le transverse les
            // abscisses. **C'est là que l'étourderie guette** — prendre les
            // bornes du mauvais axe donne une cage à la bonne place dans l'axe de
            // sa montée et décalée dans l'autre, ce qui ne se voit qu'au refus
            // d'un portail qui tombe sur la clé d'un voisin.
            Side::North => Self {
                entry: south,
                exit: north,
                sides: (east, west),
                lengthwise: false,
                base,
            },
            _ => Self {
                entry: north,
                exit: south,
                sides: (west, east),
                lengthwise: false,
                base,
            },
        }
    }

    /// Le point du monde à cette avancée, ce bord et cette cote.
    fn point(&self, run: f32, side: f32, z: f32) -> [f32; 3] {
        if self.lengthwise {
            [run, side, z]
        } else {
            [side, run, z]
        }
    }

    /// Le sens de la montée, de l'entrée vers la sortie.
    fn direction(&self) -> f32 {
        (self.exit - self.entry).signum()
    }

    /// La direction unitaire de la montée, axe horizontal des flancs.
    fn uphill(&self) -> [f32; 3] {
        self.point(self.direction(), 0.0, 0.0)
    }

    /// Celle du transverse, axe horizontal des contremarches et du linteau.
    fn across(&self) -> [f32; 3] {
        self.point(0.0, (self.sides.1 - self.sides.0).signum(), 0.0)
    }

    /// La direction **unitaire** d'un segment du profil, axe de pente d'une rampe.
    ///
    /// Unitaire et non le segment lui-même, comme [`ALONG`] : son carré vaut un,
    /// donc le carré de l'axe à l'échelle reste celui des faces droites quelle
    /// que soit la pente, et la densité de luxels le long de la rampe est celle
    /// du reste du décor. Prendre le segment donnerait une marche d'éclairage à
    /// chaque jointure.
    fn slope(&self, run: f32, rise: f32) -> [f32; 3] {
        let length = run.hypot(rise);
        self.point(run / length, 0.0, rise / length)
    }

    /// Le profil du sol dans le plan de la montée, de l'entrée vers la sortie.
    ///
    /// **Deux sommets consécutifs bornent toujours une surface** — horizontale
    /// quand ils partagent leur cote, verticale quand ils partagent leur avancée,
    /// oblique sinon —, et c'est ce qui range le sol d'une cage en une seule
    /// boucle, sans distinguer sa forme.
    ///
    /// La sortie vient de `inner_high` ou `inner_low` et non du cumul des
    /// marches : c'est elle que le portail du haut doit porter au bit près.
    fn profile(&self, shape: Shape) -> Vec<(f32, f32)> {
        let mut profile = Vec::with_capacity(2 * STEPS as usize + 2);
        profile.push((self.entry, self.base));
        if shape == Shape::Steps {
            for i in 0..STEPS {
                let run = self.entry + self.direction() * (LANDING + i as f32 * TREAD);
                profile.push((run, self.base + i as f32 * RISE));
                profile.push((run, self.base + (i + 1) as f32 * RISE));
            }
        }
        profile.push((self.exit, self.base + LEVEL));
        profile
    }

    /// Les deux axes du segment qui joint ces deux sommets du profil.
    fn facet(&self, from: (f32, f32), to: (f32, f32)) -> ([f32; 3], [f32; 3]) {
        if from.1 == to.1 {
            FLAT
        } else if from.0 == to.0 {
            (self.across(), UPWARD)
        } else {
            (self.across(), self.slope(to.0 - from.0, to.1 - from.1))
        }
    }

    /// L'aire que le sol retire à la section verticale de la cage.
    ///
    /// Sous les marches, la somme des rectangles, dont les hauteurs se cumulent
    /// en `STEPS(STEPS + 1) / 2` ; sous une rampe, le triangle de la case entière.
    fn carved(shape: Shape) -> f64 {
        match shape {
            Shape::Steps => f64::from(RISE) * f64::from(TREAD) * f64::from(STEPS * (STEPS + 1) / 2),
            Shape::Ramp => f64::from(INNER) * f64::from(LEVEL) / 2.0,
        }
    }
}

/// Une surface de cage, avant son écriture.
///
/// **Rassemblées plutôt qu'écrites au fil de l'eau** : le contrôle de fermeture a
/// besoin de toutes, et il doit passer avant qu'un octet ne sorte.
struct Face {
    /// Les indices de ses sommets dans la cellule.
    indices: Vec<u32>,
    /// L'emplacement de son matériau.
    material: u32,
    /// Ses deux axes, à l'échelle près : la densité s'applique à l'écriture.
    frame: ([f32; 3], [f32; 3]),
}

/// La cellule d'une cage d'escalier : deux cases superposées, un sol en marches.
///
/// **Une seule cellule pour deux cases**, et c'est ce que la non-convexité
/// permet : le creux au-dessus des marches reste dans le volume, ce qui rend la
/// traversée conservatrice sans jamais la rendre fausse. La case du dessus n'a
/// donc pas de cellule à elle, et son identifiant reste inemployé.
fn stair(grid: &Grid, flight: Stair, out: &mut Vec<u8>) {
    let (width, height, levels) = grid.extent();
    let cells = width * height * levels;
    let rank = cell_id(grid, flight.foot) - 1;
    let frame = Flight::new(flight.foot, flight.climb);
    let top = frame.base + LEVEL + CEILING;

    let profile = frame.profile(flight.shape);

    let mut points = Vec::with_capacity(2 * profile.len() + 6);
    for &(run, z) in &profile {
        points.push(frame.point(run, frame.sides.0, z));
        points.push(frame.point(run, frame.sides.1, z));
    }
    for (run, z) in [
        (frame.entry, top),
        (frame.exit, top),
        (frame.entry, frame.base + CEILING),
    ] {
        points.push(frame.point(run, frame.sides.0, z));
        points.push(frame.point(run, frame.sides.1, z));
    }

    let arrival = 2 * (profile.len() as u32 - 1);
    let ceiling = 2 * profile.len() as u32;
    let departure = ceiling + 2;
    let lintel = ceiling + 4;

    // Le même ordre d'indices pour les trois natures de segment, et ce n'est pas
    // une coïncidence : pris de l'entrée vers la sortie puis d'un bord à
    // l'autre, il donne une normale vers le haut sur une marche ou sur une
    // rampe, et vers l'entrée sur une contremarche — l'intérieur du volume dans
    // les trois cas. L'inclinaison décide du repère et non du matériau : une
    // cage est d'une seule matière, celle qu'on foule, et c'est la contremarche
    // qu'on voit de face en montant.
    let mut faces = Vec::with_capacity(match flight.shape {
        Shape::Steps => STAIR_SURFACES,
        Shape::Ramp => RAMP_SURFACES,
    } as usize);
    for k in 0..profile.len() as u32 - 1 {
        faces.push(Face {
            indices: vec![2 * k, 2 * k + 2, 2 * k + 3, 2 * k + 1],
            material: FLOOR,
            frame: frame.facet(profile[k as usize], profile[k as usize + 1]),
        });
    }

    faces.push(Face {
        indices: vec![ceiling + 1, departure + 1, departure, ceiling],
        material: FLOOR,
        frame: FLAT,
    });

    // Les flancs suivent le profil : vingt-huit sommets en dents de scie sous
    // des marches, quatre sous une rampe, là où le format en accepte
    // soixante-quatre par polygone. Celui du premier bord se parcourt à rebours
    // pour que sa normale regarde l'autre.
    let mut near = vec![ceiling, departure];
    let mut far = Vec::with_capacity(profile.len() + 2);
    for k in 0..profile.len() as u32 {
        near.push(2 * (profile.len() as u32 - 1 - k));
        far.push(2 * k + 1);
    }
    far.push(departure + 1);
    far.push(ceiling + 1);
    for indices in [near, far] {
        faces.push(Face {
            indices,
            material: WALL,
            frame: (frame.uphill(), UPWARD),
        });
    }

    // Le linteau : le portail d'entrée a la hauteur d'un passage dans une cage
    // qui fait deux étages, et ce qui reste au-dessus est du mur. Ses deux
    // sommets du bas sont les sommets de coupure, sans lesquels le portail
    // n'aurait aucun sommet commun avec celui du passage — et le chargement en
    // ferait deux murs.
    faces.push(Face {
        indices: vec![lintel + 1, ceiling + 1, ceiling, lintel],
        material: WALL,
        frame: (frame.across(), UPWARD),
    });

    let gates = [
        vec![0, lintel, lintel + 1, 1],
        vec![arrival + 1, departure + 1, departure, arrival],
    ];
    closed(rank + 1, &faces, &gates, &points, flight.shape);

    let mut body = Vec::new();
    words(
        &[
            cell_id(grid, flight.foot),
            0,
            points.len() as u32,
            faces.len() as u32,
            gates.len() as u32,
        ],
        &mut body,
    );
    for point in &points {
        floats(point, &mut body);
    }

    let surfaces = cells * 3 * PRISM_SURFACES + rank * STAIR_SURFACES;
    for (slot, face) in faces.iter().enumerate() {
        surface(
            surfaces + 1 + slot as u32,
            face.material,
            &face.indices,
            &points,
            face.frame,
            &mut body,
        );
    }

    let portals = cells * 3 * PRISM_PORTALS + rank * STAIR_PORTALS;
    for (slot, indices) in gates.iter().enumerate() {
        words(
            &[portals + 1 + slot as u32, indices.len() as u32],
            &mut body,
        );
        words(indices, &mut body);
    }

    words(&[body.len() as u32], out);
    out.extend_from_slice(&body);
}

/// Vérifie qu'une cage est un volume fermé, et du bon côté.
///
/// **Six fois le volume signé**, par la divergence, portails compris avec leur
/// enroulement rentré : négatif puisque les surfaces regardent vers l'intérieur.
/// Comparé à sa valeur géométrique, ce nombre attrape les deux fautes qu'une
/// cellule à vingt-neuf surfaces écrites à la main rend probables — une face à
/// l'envers, qui change le signe de sa part, et une face oubliée, qui laisse le
/// volume ouvert. Ni l'une ni l'autre ne lève d'erreur au chargement : l'écran
/// montre du fond, et rien ne dit où.
///
/// **C'est aussi le seul contrôle qui distingue les deux formes de cage**, et il
/// le fait par la seule aire que leur sol retire : une rampe en ôte plus que les
/// marches qu'elle remplace, puisqu'elle passe sous leurs nez.
fn closed(id: u32, faces: &[Face], gates: &[Vec<u32>], points: &[[f32; 3]], shape: Shape) {
    let shell: Vec<Vec<u32>> = faces
        .iter()
        .map(|face| face.indices.clone())
        .chain(
            gates
                .iter()
                .map(|indices| indices.iter().rev().copied().collect()),
        )
        .collect();
    let six = six_volume(&shell, points);

    // Le volume exact : la section verticale fois la largeur, la section étant
    // la cage pleine moins ce que son sol lui prend.
    let section = f64::from(INNER) * f64::from(LEVEL + CEILING) - Flight::carved(shape);
    let expected = -6.0 * section * f64::from(INNER);
    assert!(
        (six - expected).abs() <= expected.abs() * 1e-6,
        "la cage {id} enferme {} au lieu de {expected}",
        six
    );
}

/// Six fois le volume signé d'une coque de polygones, par la divergence.
///
/// **Négatif pour une cellule bien écrite**, dont les surfaces regardent toutes
/// vers l'intérieur. Un signe positif dit que la coque entière est retournée, et
/// c'est une faute que ni le chargement ni l'image ne signalent : chaque face
/// prise isolément reste correctement enroulée, et c'est le volume d'ensemble qui
/// décide de la normale que la collision emploie.
fn six_volume(polygons: &[Vec<u32>], points: &[[f32; 3]]) -> f64 {
    let mut six = 0.0f64;
    for indices in polygons {
        for k in 1..indices.len() - 1 {
            let a = local(points[indices[0] as usize], points[0]);
            let b = local(points[indices[k] as usize], points[0]);
            let c = local(points[indices[k + 1] as usize], points[0]);
            six += a[0] * (b[1] * c[2] - b[2] * c[1])
                + a[1] * (b[2] * c[0] - b[0] * c[2])
                + a[2] * (b[0] * c[1] - b[1] * c[0]);
        }
    }
    six
}

/// Un point ramené près de l'origine, en double précision.
///
/// **La translation n'est pas un détail de confort** : le volume en est
/// indépendant, mais la somme qui le calcule ne l'est pas. Une cage du bord de la
/// carte a ses sommets autour de soixante, donc des produits autour de deux cent
/// mille qui s'annulent presque tous — et le résidu d'arrondi noie le volume
/// qu'on cherche.
fn local(point: [f32; 3], origin: [f32; 3]) -> [f64; 3] {
    [
        f64::from(point[0]) - f64::from(origin[0]),
        f64::from(point[1]) - f64::from(origin[1]),
        f64::from(point[2]) - f64::from(origin[2]),
    ]
}

/// Un prisme droit : son empreinte extrudée, ses murs, et un portail par côté
/// ouvert.
fn prism(
    id: u32,
    surfaces: u32,
    portals: u32,
    footprint: [[f32; 2]; 4],
    level: u32,
    open: [bool; 4],
    out: &mut Vec<u8>,
) {
    let (low, high) = (floor_of(level), floor_of(level) + CEILING);

    // L'empreinte est écrite ici et ne vient de personne, mais une main qui en
    // changerait l'ordre retournerait **toutes** les surfaces de la cellule d'un
    // coup : la cellule deviendrait une coquille vue de dehors, et l'image
    // montrerait du fond là où le décor devrait être. Le signe se vérifie donc.
    assert!(
        twice_area(&footprint) > 0.0,
        "l'empreinte de la cellule {id} tourne à l'envers"
    );

    let mut points = Vec::with_capacity(8);
    for z in [low, high] {
        for point in footprint {
            points.push([point[0], point[1], z]);
        }
    }

    let walls: Vec<usize> = (0..4).filter(|i| !open[*i]).collect();
    let gates: Vec<usize> = (0..4).filter(|i| open[*i]).collect();

    // **Le même contrôle de volume signé qu'une cage**, et il manquait ici. Un
    // prisme n'a que six faces, mais l'inversion qu'il cache est la même : chaque
    // face reste correctement enroulée prise à part, l'image est donc juste, et
    // c'est la coque entière qui décide de la normale intérieure dont la
    // collision se sert. Un signe faux rend la cellule traversable sans que rien
    // ne le dise.
    //
    // Les quatre côtés comptent comme des murs, ouverts ou non : un portail prend
    // l'enroulement inverse de la surface qu'il remplace, donc rentré il la
    // retrouve — un volume se ferme de ses six faces, quelle que soit la nature
    // de chacune.
    let mut shell = vec![vec![0, 1, 2, 3], vec![7, 6, 5, 4]];
    for i in 0..4u32 {
        let j = (i + 1) % 4;
        shell.push(vec![i, i + 4, j + 4, j]);
    }
    let six = six_volume(&shell, &points);
    let expected = -3.0 * f64::from(twice_area(&footprint)) * f64::from(high - low);
    assert!(
        (six - expected).abs() <= expected.abs() * 1e-6,
        "la cellule {id} enferme {six} au lieu de {expected}"
    );

    let mut body = Vec::new();
    words(
        &[
            id,
            0,
            points.len() as u32,
            walls.len() as u32 + 2,
            gates.len() as u32,
        ],
        &mut body,
    );
    for point in &points {
        floats(point, &mut body);
    }

    // Le sol dans l'ordre de l'empreinte, le plafond à l'envers : écrits dans le
    // même sens, l'un des deux serait un dos de face et disparaîtrait.
    surface(surfaces + 1, FLOOR, &[0, 1, 2, 3], &points, FLAT, &mut body);
    surface(surfaces + 2, FLOOR, &[7, 6, 5, 4], &points, FLAT, &mut body);
    for (slot, &i) in walls.iter().enumerate() {
        let j = (i + 1) % 4;
        let indices = [i as u32, (i + 4) as u32, (j + 4) as u32, j as u32];
        surface(
            surfaces + 3 + slot as u32,
            WALL,
            &indices,
            &points,
            (ALONG[i], UPWARD),
            &mut body,
        );
    }

    // Un portail prend l'enroulement inverse de la surface qu'il remplace, et la
    // cellule d'en face écrit le même quadrilatère depuis sa propre arête.
    for (slot, &i) in gates.iter().enumerate() {
        let j = (i + 1) % 4;
        words(&[portals + 1 + slot as u32, 4], &mut body);
        words(
            &[i as u32, j as u32, (j + 4) as u32, (i + 4) as u32],
            &mut body,
        );
    }

    words(&[body.len() as u32], out);
    out.extend_from_slice(&body);
}

/// Une surface : ses indices, puis son repère de texture et celui de lightmap.
fn surface(
    id: u32,
    material: u32,
    indices: &[u32],
    points: &[[f32; 3]],
    frame: ([f32; 3], [f32; 3]),
    out: &mut Vec<u8>,
) {
    let (along, across) = frame;
    words(&[id, 0, material, indices.len() as u32], out);
    words(indices, out);
    mapping(scaled(along, TEXELS), scaled(across, TEXELS), out);
    mapping(scaled(along, LUXELS), scaled(across, LUXELS), out);

    check(id, indices, points, scaled(along, TEXELS));
    check(id, indices, points, scaled(across, TEXELS));
    planar(id, indices, points, along, across);
    for (axis, limit) in [
        (scaled(along, TEXELS), 16_384.0),
        (scaled(across, TEXELS), 16_384.0),
        (scaled(along, LUXELS), 256.0),
        (scaled(across, LUXELS), 256.0),
    ] {
        span(id, indices, points, axis, limit);
    }
}

/// Vérifie que l'étendue d'une surface sur un axe tient sous son plafond.
///
/// Deux plafonds, et le chargement les refuse tous deux sans nommer la surface :
/// 256 luxels par côté, et 16 384 texels après le repli — qui ramène le minimum
/// dans la fenêtre, pas la surface entière.
fn span(id: u32, indices: &[u32], points: &[[f32; 3]], axis: [f32; 3], limit: f32) {
    let mut low = f32::MAX;
    let mut high = f32::MIN;
    for &index in indices {
        let point = points[index as usize];
        let coordinate = point[0] * axis[0] + point[1] * axis[1] + point[2] * axis[2];
        if coordinate < low {
            low = coordinate;
        }
        if coordinate > high {
            high = coordinate;
        }
    }
    assert!(
        high - low <= limit,
        "la surface {id} s'étend sur {} le long de {axis:?}, au-delà de {limit}",
        high - low
    );

    // Le chargement replie les coordonnées de texture d'un multiple entier de la
    // plus grande planche, pour que le minimum rentre dans la fenêtre. Ce qui
    // reste dehors après ce repli est refusé, et l'étendue seule ne le dit pas :
    // le minimum replié s'y ajoute.
    if limit > 256.0 {
        let folded = low - (low / 2048.0).floor() * 2048.0;
        assert!(
            folded + (high - low) <= limit,
            "la surface {id} sort de la fenêtre après repli : {} le long de {axis:?}",
            folded + (high - low)
        );
    }
}

/// Vérifie que les deux axes d'un repère tiennent dans le plan de la surface.
///
/// Le chargement l'exige, par la normale de Newell du polygone, et le refus qu'il
/// rend ne nomme pas la surface. Avec vingt-neuf surfaces par cage dans quatre
/// orientations, c'est la faute la plus probable et la plus coûteuse à chercher.
fn planar(id: u32, indices: &[u32], points: &[[f32; 3]], along: [f32; 3], across: [f32; 3]) {
    /// La tolérance du chargement, relative et sur le carré du produit scalaire.
    const TOLERANCE: f64 = 1.0 / 1_048_576.0;

    // **En simple précision et par la somme de Newell, comme le chargement** :
    // le résidu d'arrondi d'un polygone à vingt-huit sommets loin de l'origine
    // est précisément ce qu'on cherche à voir, et le calculer en double le
    // ferait disparaître.
    let mut normal = [0.0f32; 3];
    for slot in 0..indices.len() {
        let a = points[indices[slot] as usize];
        let b = points[indices[(slot + 1) % indices.len()] as usize];
        normal[0] += (a[1] - b[1]) * (a[2] + b[2]);
        normal[1] += (a[2] - b[2]) * (a[0] + b[0]);
        normal[2] += (a[0] - b[0]) * (a[1] + b[1]);
    }

    let square = f64::from(normal[0]) * f64::from(normal[0])
        + f64::from(normal[1]) * f64::from(normal[1])
        + f64::from(normal[2]) * f64::from(normal[2]);
    for axis in [along, across] {
        let dot = f64::from(normal[0]) * f64::from(axis[0])
            + f64::from(normal[1]) * f64::from(axis[1])
            + f64::from(normal[2]) * f64::from(axis[2]);
        let unit = f64::from(axis[0]) * f64::from(axis[0])
            + f64::from(axis[1]) * f64::from(axis[1])
            + f64::from(axis[2]) * f64::from(axis[2]);
        assert!(
            dot * dot <= TOLERANCE * TOLERANCE * square * unit,
            "l'axe {axis:?} de la surface {id} sort de son plan, de normale {normal:?}"
        );
    }
}

/// Un repère : l'origine, puis les deux axes.
///
/// L'origine est le zéro du monde, toujours. Le chargement exige qu'elle tombe
/// sur un nœud de sa propre grille de luxels, et le zéro y est par construction
/// — c'est le geste qui dispense de tout rabattement.
///
/// **Le carré des axes n'est pas contrôlé** : un axe **unitaire oblique** ne peut
/// pas tomber sur une puissance de deux — à 45°, le sien passe à quelques ulp de
/// seize à l'échelle des luxels, jamais dessus —, et le chargement ne le demande
/// pas. Ce qu'un tel contrôle attraperait d'un repère faux, `planar` et `span` le
/// voient déjà.
fn mapping(u: [f32; 3], v: [f32; 3], out: &mut Vec<u8>) {
    floats(&[0.0, 0.0, 0.0], out);
    floats(&u, out);
    floats(&v, out);
}

/// Vérifie qu'aucune coordonnée dérivée ne dépasse ce que le chargement accepte.
///
/// Cette borne n'est écrite nulle part dans la documentation des cartes, et le
/// décor le plus riche du moteur est **pile dessus** : un mur d'une unité de plus
/// y ferait refuser le fichier entier, sans que rien ne dise laquelle des
/// surfaces est en cause.
fn check(id: u32, indices: &[u32], points: &[[f32; 3]], axis: [f32; 3]) {
    /// La plus grande coordonnée de texture que le chargement accepte.
    const LIMIT: f32 = 16_384.0;

    for &index in indices {
        let point = points[index as usize];
        let coordinate = point[0] * axis[0] + point[1] * axis[1] + point[2] * axis[2];
        assert!(
            coordinate.abs() <= LIMIT,
            "la surface {id} porte une coordonnée de {coordinate}"
        );
    }
}

/// L'unité de monde d'un rang de case, sur un axe horizontal.
///
/// **Une seule fonction pour les deux côtés d'une arête**, et c'est ce qui tient
/// l'appariement : deux cellules voisines y passent le même rang et reçoivent les
/// mêmes bits. Le format n'accorde aucune tolérance, et un epsilon n'y
/// changerait rien — la relation cesserait d'être transitive.
fn coord(index: u32) -> f32 {
    index as f32 * CELL
}

/// La borne basse d'une cellule de case, sur un axe horizontal.
fn inner_low(index: u32) -> f32 {
    coord(index) + MARGIN
}

/// Sa borne haute.
///
/// Écrite depuis la frontière de la case suivante et non depuis la sienne : un
/// passage se calcule des deux côtés de cette même frontière, et les trois
/// expressions doivent rendre les mêmes bits.
fn inner_high(index: u32) -> f32 {
    coord(index + 1) - MARGIN
}

/// La cote du sol d'un étage.
fn floor_of(level: u32) -> f32 {
    level as f32 * LEVEL
}

/// Le double de l'aire signée d'une empreinte.
fn twice_area(footprint: &[[f32; 2]]) -> f32 {
    let mut total = 0.0;
    for i in 0..footprint.len() {
        let (a, b) = (footprint[i], footprint[(i + 1) % footprint.len()]);
        total += a[0] * b[1] - b[0] * a[1];
    }
    total
}

/// Un axe à l'échelle d'une densité.
fn scaled(axis: [f32; 3], density: f32) -> [f32; 3] {
    [axis[0] * density, axis[1] * density, axis[2] * density]
}

/// Ajoute des flottants, octet de poids faible en tête.
fn floats(values: &[f32], out: &mut Vec<u8>) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}

/// Ajoute des entiers, de la même façon.
fn words(values: &[u32], out: &mut Vec<u8>) {
    for value in values {
        out.extend_from_slice(&value.to_le_bytes());
    }
}
