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

use super::grid::{Grid, Side};

#[cfg(test)]
mod tests;

/// Le côté d'une case, en unités de monde — une unité vaut un mètre.
pub const CELL: f32 = 4.0;

/// La hauteur d'un étage, du sol d'un niveau au sol du suivant.
///
/// Égale au côté d'une case, et ce n'est pas une coïncidence : c'est ce qui
/// permettra à une rampe de monter à **45° exactement**, seul angle dont l'axe de
/// lightmap s'écrive `(p, 0, p)` — de carré `2p²`, donc une puissance de deux.
/// Une pente quelconque ferait refuser le fichier entier.
pub const LEVEL: f32 = 4.0;

/// La hauteur sous plafond, ce qui laisse une dalle entre deux étages.
pub const CEILING: f32 = 2.4;

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

/// L'emplacement du matériau des murs.
const WALL: u32 = 1;

/// Celui du sol et du plafond.
const FLOOR: u32 = 2;

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
    let cells = cells(grid);
    let materials = materials();

    // Les genres se rangent par ordre croissant et pavent le fichier sans trou :
    // `CELL` avant `MATS`, et la première section commence juste après la table.
    let first = HEADER + ENTRY * 2;
    let total = first + cells.len() + materials.len();

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&SIGNATURE);
    out.extend_from_slice(b"WRLD");
    words(&[VERSION, total as u32, 2], &mut out);

    let mut offset = first;
    for (tag, body) in [(b"CELL", &cells), (b"MATS", &materials)] {
        out.extend_from_slice(tag);
        words(&[offset as u32, body.len() as u32], &mut out);
        offset += body.len();
    }

    out.extend_from_slice(&cells);
    out.extend_from_slice(&materials);
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

/// La section des cellules, une par case.
fn cells(grid: &Grid) -> Vec<u8> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::new();
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                cell(grid, (x, y, z), cell_id(grid, (x, y, z)) - 1, &mut out);
            }
        }
    }
    out
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

/// Une cellule de case : son empreinte extrudée, ses murs et ses portails.
fn cell(grid: &Grid, at: (u32, u32, u32), rank: u32, out: &mut Vec<u8>) {
    let (low, high) = (floor_of(at.2), floor_of(at.2) + CEILING);
    let (x0, x1) = (coord(at.0), coord(at.0 + 1));
    let (y0, y1) = (coord(at.1), coord(at.1 + 1));
    let footprint = [[x0, y0], [x1, y0], [x1, y1], [x0, y1]];

    // L'empreinte est écrite ici et ne vient de personne, mais une main qui en
    // changerait l'ordre retournerait **toutes** les surfaces de la cellule d'un
    // coup : la cellule deviendrait une coquille vue de dehors, et l'image
    // montrerait du fond là où le décor devrait être. Le signe se vérifie donc.
    assert!(
        twice_area(&footprint) > 0.0,
        "l'empreinte de {at:?} tourne à l'envers"
    );

    let mut points = Vec::with_capacity(8);
    for z in [low, high] {
        for point in footprint {
            points.push([point[0], point[1], z]);
        }
    }

    let walls: Vec<usize> = (0..4).filter(|i| grid.has_wall(at, EDGES[*i])).collect();
    let gates: Vec<usize> = (0..4).filter(|i| !grid.has_wall(at, EDGES[*i])).collect();

    let mut body = Vec::new();
    words(
        &[
            rank + 1,
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
    let surfaces = rank * 6;
    surface(surfaces + 1, FLOOR, &[0, 1, 2, 3], &points, FLAT, &mut body);
    surface(surfaces + 2, FLOOR, &[7, 6, 5, 4], &points, FLAT, &mut body);
    for (slot, &i) in walls.iter().enumerate() {
        let j = (i + 1) % 4;
        let indices = [i as u32, (i + 4) as u32, (j + 4) as u32, j as u32];
        let frame = (ALONG[i], UPWARD);
        surface(
            surfaces + 3 + slot as u32,
            WALL,
            &indices,
            &points,
            frame,
            &mut body,
        );
    }

    // Un portail prend l'enroulement inverse de la surface qu'il remplace, et la
    // cellule d'en face écrit le même quadrilatère depuis sa propre arête. Les
    // deux jeux de positions sortent de `coord`, donc ils portent les mêmes bits
    // — ce que l'appariement exige, sans la moindre tolérance.
    for (slot, &i) in gates.iter().enumerate() {
        let j = (i + 1) % 4;
        words(&[rank * 4 + 1 + slot as u32, 4], &mut body);
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
}

/// Un repère : l'origine, puis les deux axes.
///
/// L'origine est le zéro du monde, toujours. Le chargement exige qu'elle tombe
/// sur un nœud de sa propre grille de luxels, et le zéro y est par construction
/// — c'est le geste qui dispense de tout rabattement.
fn mapping(u: [f32; 3], v: [f32; 3], out: &mut Vec<u8>) {
    assert!(
        power_of_two(square(v)) && power_of_two(square(u)),
        "un axe de repère n'a pas un carré en puissance de deux : {u:?} {v:?}"
    );
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

/// Le carré de la longueur d'un axe.
fn square(axis: [f32; 3]) -> f32 {
    axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]
}

/// Vrai pour une puissance de deux, mantisse nulle et valeur utilisable.
///
/// L'exposant n'a pas à être pair : un axe oblique a un carré de la forme `2p²`,
/// et l'exiger interdirait tout mur en biais.
fn power_of_two(value: f32) -> bool {
    value > 0.0 && value.is_finite() && value.to_bits() & 0x007f_ffff == 0
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
