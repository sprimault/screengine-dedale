// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du plan de contrôle.
//!
//! **Des prédicats sur le tampon, jamais une empreinte d'image.** Ce qu'on compare
//! ici sont des pixels que le jeu a posés lui-même en coordonnées d'écran ; la
//! conformance du rendu est l'affaire du moteur, qui la tient sur ses propres
//! scènes, et une seconde suite d'empreintes casserait à chaque changement
//! d'habillage.
//!
//! Rien n'y charge de carte : le plan ne lit que la grille, donc aucune épreuve
//! d'ici n'a besoin d'un contexte de rendu.

use super::*;
use crate::test_support::{Canvas, HEIGHT, WIDTH};
use maze::grid::Grid;

/// Une zone du tampon, bornes incluses.
struct Area {
    /// Son abscisse la plus faible.
    left: u32,
    /// Son ordonnée la plus faible, donc son bord nord à l'écran.
    top: u32,
    /// Son abscisse la plus forte.
    right: u32,
    /// Son ordonnée la plus forte.
    bottom: u32,
}

impl Area {
    /// Sa largeur en pixels.
    fn width(&self) -> u32 {
        self.right - self.left + 1
    }

    /// Sa hauteur en pixels.
    fn height(&self) -> u32 {
        self.bottom - self.top + 1
    }
}

/// Le labyrinthe du jeu, pris à ses réglages.
///
/// La graine est celle que le jeu lance, donc un défaut vu à l'écran se retrouve
/// ici sur le même décor.
fn maze() -> Grid {
    Grid::generate(MAZE)
}

/// Un tampon neuf, et ce qu'on y dessine.
fn draw(paint: impl FnOnce(&mut Output<'_>)) -> Canvas {
    let mut canvas = Canvas::new(WIDTH, HEIGHT);
    paint(&mut canvas.output());
    canvas
}

/// La zone qu'occupe une teinte entre deux abscisses, ou `None` si aucun pixel ne
/// la porte.
///
/// **La fenêtre en abscisse sert à isoler un plan de son voisin** : deux cases
/// d'une même volée portent la même teinte sur deux étages, et une zone qui les
/// engloberait n'aurait plus de bord plein à montrer.
fn bounds(canvas: &Canvas, colour: [u8; 4], left: u32, right: u32) -> Option<Area> {
    let mut found: Option<Area> = None;
    for y in 0..canvas.height() {
        for x in left..=right {
            if canvas.pixel(x, y) != colour {
                continue;
            }
            found = Some(match found {
                None => Area {
                    left: x,
                    top: y,
                    right: x,
                    bottom: y,
                },
                Some(area) => Area {
                    left: area.left.min(x),
                    top: area.top.min(y),
                    right: area.right.max(x),
                    bottom: area.bottom.max(y),
                },
            });
        }
    }
    found
}

/// Le bord d'une zone où la teinte occupe toute la rangée, s'il y en a un seul.
///
/// **C'est la base d'un triangle qu'on cherche**, et elle est son seul bord plein :
/// les rangées suivantes raccourcissent vers la pointe. Un carré plein en aurait
/// quatre, et la fonction rend alors `None` plutôt que d'en élire un.
fn base(canvas: &Canvas, area: &Area, colour: [u8; 4]) -> Option<Side> {
    let column = |x: u32| (area.top..=area.bottom).all(|y| canvas.pixel(x, y) == colour);
    let row = |y: u32| (area.left..=area.right).all(|x| canvas.pixel(x, y) == colour);

    // Le nord est en haut de l'écran, donc son bord est celui des ordonnées
    // faibles.
    let sides = [
        (Side::West, column(area.left)),
        (Side::East, column(area.right)),
        (Side::North, row(area.top)),
        (Side::South, row(area.bottom)),
    ];
    let mut only = None;
    for (side, full) in sides {
        if !full {
            continue;
        }
        if only.is_some() {
            return None;
        }
        only = Some(side);
    }
    only
}

/// La marque « ici » d'une case, quand elle en désigne une seule.
///
/// **`None` pour une case de cage**, et c'est voulu : les deux cases d'une volée
/// partagent sa cellule, donc la marque s'allume sur deux plans à la fois et sa
/// zone ne situe plus rien. Une case ordinaire rend un carré du côté de la case,
/// moins ses deux bords.
fn here(grid: &Grid, cell: (u32, u32, u32)) -> Option<Area> {
    let canvas = draw(|output| plot(output, grid, export::cover(grid, cell)));
    let area = bounds(&canvas, HERE, 0, canvas.width() - 1)?;
    let side = CELL - 4;
    (area.width() == side && area.height() == side).then_some(area)
}

/// L'abscisse du plan d'un étage, mesurée par une de ses cases ordinaires.
///
/// Prend la première case de l'étage que `here` situe : il y en a toujours, les
/// volées étant six sur deux cent cinquante-six cases.
fn plane(grid: &Grid, level: u32) -> u32 {
    let (width, height, _) = grid.extent();
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y, level)))
        .find_map(|cell| here(grid, cell))
        .map(|area| area.left)
        .expect("un étage a des cases hors cage")
}

/// La pointe d'une rampe va vers la montée, et sa base occupe le bord opposé.
///
/// **Le défaut qui a tenu une version** : les quatre bras posaient la base du côté
/// vers lequel ils devaient pointer, donc le repère désignait la descente, et rien
/// ne gardait l'arithmétique du plan.
#[test]
fn une_rampe_pointe_vers_sa_montee() {
    let side = CELL - 2;
    for towards in [Side::East, Side::West, Side::North, Side::South] {
        let canvas = draw(|output| wedge(output, 4, 4, side, towards, WALL));
        let area = bounds(&canvas, WALL, 0, canvas.width() - 1).expect("le triangle est peint");
        assert_eq!(
            base(&canvas, &area, WALL),
            Some(towards.facing()),
            "la base d'une pointe vers {towards:?} occupe le bord opposé"
        );
    }
}

/// Les deux cases d'une volée en rampe portent des pointes opposées, celle du pied
/// vers la montée.
///
/// C'est ce qui distingue les deux cases d'une cage sur le plan, là où un escalier
/// s'en sort par ses demi-cases. La teinte ne peut pas le dire : elle est prise par
/// l'appariement d'un plan à l'autre.
#[test]
fn les_deux_bouts_d_une_rampe_s_opposent() {
    let grid = maze();
    let rank = grid
        .stairs()
        .iter()
        .position(|stair| stair.shape == Shape::Ramp)
        .expect("les réglages demandent deux rampes");
    let stair = grid.stairs()[rank];
    let colour = LINKS[rank % LINKS.len()];

    // Aucune case ne porte la marque « ici », qui recouvrirait un triangle.
    let canvas = draw(|output| plot(output, &grid, u32::MAX));
    let whole = bounds(&canvas, colour, 0, canvas.width() - 1).expect("la volée est marquée");

    // Les deux cases d'une cage ont le même rang dans leurs étages, donc leurs
    // triangles sont symétriques de part et d'autre de l'écart entre plans : la
    // coupure se prend au milieu de la zone plutôt qu'en recopiant l'adressage.
    let split = (whole.left + whole.right) / 2;
    for (level, climb) in [
        (stair.foot.2, stair.climb),
        (stair.head().2, stair.climb.facing()),
    ] {
        let (left, right) = if plane(&grid, level) <= split {
            (whole.left, split)
        } else {
            (split + 1, whole.right)
        };
        let area = bounds(&canvas, colour, left, right).expect("le triangle de l'étage est peint");
        assert_eq!(
            base(&canvas, &area, colour),
            Some(climb.facing()),
            "la case de l'étage {level} pointe vers {climb:?}"
        );
    }
}

/// Le nord d'une colonne de cases se dessine au-dessus de son sud.
///
/// **La ligne s'inverse en passant à l'écran**, et c'est le genre d'inversion qu'on
/// relit cent fois sans la voir : deux cases de la même colonne suffisent à la
/// garder, la plus au nord devant être la plus haute.
#[test]
fn le_nord_est_en_haut_du_plan() {
    let grid = maze();
    let (width, height, _) = grid.extent();
    let pair = (0..width).find_map(|x| {
        // Par les deux bouts : la première case ordinaire au sud, la première au
        // nord, sans tracer le plan pour toutes celles du milieu.
        let mut ordinary = (0..height).filter_map(|y| here(&grid, (x, y, 0)));
        let south = ordinary.next()?;
        let north = ordinary.next_back()?;
        Some((north, south))
    });
    let (north, south) = pair.expect("une colonne porte deux cases hors cage");
    assert!(north.top < south.top);
}

/// L'étage le plus haut occupe le plan le plus à gauche.
///
/// On lit le plan de la gauche vers la droite comme on lit une coupe de haut en
/// bas ; l'ordre inverse mettrait le départ du côté opposé à celui où l'œil arrive.
#[test]
fn l_etage_le_plus_haut_est_a_gauche() {
    let grid = maze();
    let (_, _, levels) = grid.extent();
    assert!(levels > 1, "le décor du jeu a plusieurs étages");
    assert!(plane(&grid, levels - 1) < plane(&grid, 0));
}

/// Un rectangle avance d'un pas de ligne, et non d'une largeur.
///
/// **La garde est pour le plan, pas pour le moteur** : `block` s'en remet
/// aujourd'hui à `Output::pixel`, mais l'interface de l'étape 5 écrira peut-être
/// dans la tranche entière pour aller vite, et c'est là que la largeur se
/// substitue au pas.
#[test]
fn un_rectangle_s_adresse_par_le_pas_de_ligne() {
    let canvas = draw(|output| block(output, 0, 1, 1, 1, WALL));
    assert_eq!(canvas.pixel(0, 1), WALL);
    assert_eq!(
        canvas.pixel(WIDTH, 0),
        [0; 4],
        "un adressage par la largeur aurait écrit là"
    );
}

/// Un rectangle qui dépasse n'écrit que la zone utile.
///
/// Le plan se pose au coin de l'écran, donc une résolution interne plus petite que
/// lui le ferait sortir — ce qui arrivera, la résolution pouvant changer en cours
/// de partie.
#[test]
fn un_rectangle_se_borne_au_tampon() {
    let canvas = draw(|output| block(output, WIDTH - 1, HEIGHT - 1, 8, 8, WALL));
    assert_eq!(canvas.pixel(WIDTH - 1, HEIGHT - 1), WALL);
    assert_eq!(canvas.pixel(WIDTH - 1, HEIGHT), [0; 4]);
    assert_eq!(canvas.pixel(WIDTH, HEIGHT - 1), [0; 4]);
}

/// Un anneau est creux.
///
/// C'est la forme qui sépare le départ et la sortie d'une volée : à cinq pixels de
/// côté, leurs teintes ne se distinguent pas de celle d'une cage, et un anneau
/// plein ramènerait la confusion.
#[test]
fn un_anneau_est_creux() {
    let side = CELL - 2;
    let canvas = draw(|output| ring(output, 4, 4, side, START));
    let area = bounds(&canvas, START, 0, canvas.width() - 1).expect("l'anneau est peint");
    assert_eq!((area.width(), area.height()), (side, side));
    for y in 5..4 + side - 1 {
        for x in 5..4 + side - 1 {
            assert_eq!(canvas.pixel(x, y), [0; 4], "l'intérieur reste vide");
        }
    }
}
