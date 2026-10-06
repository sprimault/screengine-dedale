// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le point d'entrée du jeu : les réglages, la boucle, et les trois rappels.
//!
//! **Les rappels ne portent rien.** Le pas de mise à jour délègue à l'état de la
//! partie, le rendu à `scene::submit`, et la sortie au plan de contrôle. C'est
//! voulu : la boucle ouvre une fenêtre, donc elle ne servira pas à l'image animée
//! du `README`, et tout ce qui vivrait dans un rappel serait à en sortir ce
//! jour-là.

mod body;
mod game;
mod maze;
mod player;
mod probe;
mod scene;
mod weapon;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use game::Game;
use maze::export;
use maze::grid::{Grid, Settings, Shape, Side};
use probe::Probe;
use scene::Scenery;
use screengine_play::{Error, KeyCode, Output, Play};

/// Les réglages du labyrinthe.
///
/// Seize cases de côté sur deux étages, assez pour que la traversée du moteur
/// ait de quoi éliminer. Six volées plutôt que la seule qu'un arbre exigerait :
/// un immeuble a plusieurs cages **et** des étages qu'on parcourt, et c'est
/// précisément ce que ce décor doit montrer du moteur. Chacune en trop ouvre une
/// boucle verticale, et huit raccourcis horizontaux s'y ajoutent.
///
/// Six **demandées** : une cage aveugle trois faces de chacune de ses deux cases,
/// et celle qui couperait le labyrinthe est écartée. Ces six-là tiennent.
///
/// Deux des six montent par une rampe : de quoi croiser les deux formes en se
/// promenant, sans que les marches cessent d'être ce qu'on rencontre le plus.
const MAZE: Settings = Settings {
    extent: (16, 16, 2),
    seed: 0x5EED_1A8E,
    stairs: 6,
    ramps: 2,
    loops: 8,
};

/// Ce que la boucle garde : le monde d'un côté, la partie de l'autre.
///
/// La frontière est ici et nulle part ailleurs. Le monde se recharge à chaud,
/// la partie est jetée avec lui, et rien de l'une n'entre dans l'autre.
struct Session {
    /// L'état du monde.
    scenery: Scenery,
    /// L'état de la partie.
    game: Game,
    /// Le relevé de ce que la traversée rend.
    ///
    /// **Ni du monde ni de la partie** : c'est un instrument, qui ne décide de
    /// rien et que le rendu seul alimente. Le ranger dans l'un des deux ferait
    /// passer un journal pour un état de jeu.
    probe: Probe,
}

/// Ouvre la fenêtre ; Échap ferme.
fn main() -> Result<(), Error> {
    let scenery = Scenery::new(MAZE)?;
    let game = Game::new(&scenery.maze, &scenery.map)?;

    // Le compte dans le titre, faute d'une police : c'est la seule sortie
    // textuelle du jeu avant l'étape 5. Ce total est celui de la **carte**, et
    // non ce que le budget d'image plafonne — lequel compte les triangles
    // préparés d'une image, que la traversée ne tire qu'à quelques centaines.
    // `Play::max_triangles` le relève s'il le faut ; la mesure qui le dirait
    // n'est pas prise, donc le défaut reste.
    let title = format!(
        "Dédale — {} cellules, {} triangles",
        scenery.map.cell_count(),
        scenery.map.triangle_count()
    );

    Play::new().title(&title).run_with_output(
        Session {
            scenery,
            game,
            probe: Probe::new(),
        },
        |session, tick| {
            if tick.input().pressed(KeyCode::Escape) {
                tick.exit();
            }
            session.game.step(tick, &session.scenery.map);

            // **La cote de l'œil dans le titre, et c'est ce qui rend les cotes
            // réglables** : elles ne se jugent qu'à l'écran, et on ne juge pas
            // une hauteur qu'on ne lit pas. Le reste à titre fixe — la hauteur
            // d'étage et le plafond — donne l'échelle sans avoir à la retrouver
            // dans le code.
            //
            // Le modulo tient parce que les sols sont aux multiples de la hauteur
            // d'étage : il rend donc la hauteur au-dessus du sol de l'étage où
            // l'on est, qui est la seule cote parlante. Au-dessus d'un escalier
            // il compte depuis le sol du bas, ce qui est exact et se lit.
            // La cellule avec la cote, et elle vaut autant : **zéro veut dire
            // hors de tout volume**, et c'est ce qui éteint l'image sans rien
            // dire d'autre — le moteur ne soumet alors aucune géométrie. Le plan
            // de contrôle, lui, continue de se dessiner, ce qui rend l'écran noir
            // difficile à lire autrement.
            let eye = session.game.view().camera.position.z;
            tick.set_title(&format!(
                "{title} — œil {:.2} sur {:.2} d'étage, plafond {:.2}, cellule {}, vue {:?}",
                eye.rem_euclid(export::LEVEL),
                export::LEVEL,
                export::CEILING,
                session.game.cell(),
                session.probe.seen()
            ));
        },
        |session, context| {
            // Un refus ne vient que de la capacité de triangles, et une image
            // manquante vaut mieux qu'une boucle arrêtée. Ce que la traversée
            // rend — complète, tronquée, ou hors cellule — n'est pas encore
            // relevé : il faudra le faire quand un décor approchera ses bornes.
            let view = session.game.view();
            // **Ce que la traversée rend se relève**, là où il était jeté : elle
            // dit si l'image est complète, si elle s'est arrêtée à une de ses
            // bornes — et il manque alors du décor —, ou si la pose est hors de
            // tout volume. Un décor de plusieurs centaines de cellules est le
            // premier à pouvoir les approcher.
            //
            // **Le refus se relève aussi**, et c'est même lui qu'on cherche : il ne
            // peut venir que de la capacité de triangles, et un dépassement annule
            // la soumission du décor entier — donc une image noire. Il ne fait
            // toujours pas tomber la boucle : une image manquante vaut mieux qu'un
            // arrêt.
            let shown = match scene::submit(context, &session.scenery, &view) {
                Ok(seen) => probe::State::Shown(seen),
                Err(_) => probe::State::Refused,
            };
            session.probe.note(shown, &session.game.aim());
            // **L'arme après le décor**, et c'est le seul ordre qui vaille : elle
            // est la plus proche de l'œil, donc la profondeur la laisserait gagner
            // de toute façon, mais la soumettre en dernier évite qu'un décor très
            // proche la rejette à égalité.
            //
            // Son refus se relève comme celui du décor : il vient de la même
            // capacité, et une arme qui disparaît de la main est un symptôme qu'on
            // chercherait longtemps sans la ligne qui le dit.
            let hand = weapon::submit(context, session.game.weapon(), &view.camera);
            session.probe.weapon(hand.is_err(), &session.game.aim());
        },
        overview,
    )
}

/// Le côté d'une case dans le plan de contrôle, en pixels.
const CELL: u32 = 7;

/// La marge du plan de contrôle au coin de l'écran, en pixels.
const INSET: u32 = 8;

/// L'écart entre deux étages voisins, en pixels.
const GAP: u32 = 10;

/// Le rappel de sortie : le relevé de l'image finie, puis le plan par-dessus.
///
/// **Dans cet ordre et pas l'autre.** Le relevé échantillonne le tampon pour dire
/// si l'image est noire ; les pixels du plan compteraient sinon comme des pixels
/// peints par la soumission.
fn overview(session: &mut Session, output: &mut Output<'_>) {
    // Un échantillon du tampon dit si l'image est amputée, ce qu'aucune valeur
    // rendue par la soumission ne dit. La couleur du fond vient de la scène, qui
    // seule sait ce qu'un pixel non peint porte.
    let aim = session.game.aim();
    session
        .probe
        .look(output, &session.scenery.map, &aim, scene::BACKGROUND);

    plot(output, &session.scenery.maze, aim.cell);
}

/// Le labyrinthe dessiné à plat, un étage par plan, dans le tampon de l'hôte.
///
/// **C'est un contrôle, pas la vue de dessus du jeu.** Celle-ci se trace en
/// lignes de **monde** par le moteur, à l'étape 9 ; ce plan-ci est fait de pixels
/// posés en coordonnées d'**écran** après la fin d'image, le seul endroit où une
/// interface a sa place.
///
/// **Les étages se dessinent tous** : un plan à un seul niveau rend un labyrinthe
/// 3D illisible, parce qu'il montre côte à côte deux cases que des dizaines de
/// passages séparent.
///
/// **Le plus haut à gauche**, donc les niveaux à rebours de leur rang. On lit le
/// plan de la gauche vers la droite comme on lit une coupe de haut en bas, et le
/// départ — au milieu de la grille, donc à l'étage supérieur quand il y en a deux
/// — tombe alors du côté où l'œil arrive d'abord.
///
/// **Il ne prend que la grille et la cellule où l'on est**, là où il vivait dans le
/// rappel : un plan se juge sur ses pixels, et `Output::new` les donne sans fenêtre.
/// Le repère inversé des rampes a tenu une version faute de ce découpage.
fn plot(output: &mut Output<'_>, maze: &Grid, here: u32) {
    let (width, height, levels) = maze.extent();

    // Un fond opaque sous le plan : posé à même le décor, il se confond avec
    // lui dès qu'un mur clair passe derrière.
    let span = levels * (width * CELL + GAP) - GAP;
    block(
        output,
        INSET / 2,
        INSET / 2,
        span + INSET,
        height * CELL + INSET,
        BACKDROP,
    );

    for level in 0..levels {
        let origin = INSET + (levels - 1 - level) * (width * CELL + GAP);
        for y in 0..height {
            for x in 0..width {
                let cell = (x, y, level);
                // Le nord en haut de l'écran, donc la ligne s'inverse.
                let top = INSET + (height - 1 - y) * CELL;
                let left = origin + x * CELL;

                if maze.has_wall(cell, Side::North) {
                    block(output, left, top, CELL + 1, 1, WALL);
                }
                if maze.has_wall(cell, Side::South) {
                    block(output, left, top + CELL, CELL + 1, 1, WALL);
                }
                if maze.has_wall(cell, Side::West) {
                    block(output, left, top, 1, CELL + 1, WALL);
                }
                if maze.has_wall(cell, Side::East) {
                    block(output, left + CELL, top, 1, CELL + 1, WALL);
                }

                mark(output, maze, cell, here, left, top);
            }
        }
    }
}

/// Ce qu'une case porte au-delà de ses murs : où l'on est, l'entrée, la sortie,
/// ou les passages d'étage qu'elle ouvre.
///
/// **Chaque passage a sa teinte, et c'est la même de ses deux côtés.** La
/// position ne suffit pas à apparier : deux plans voisins se comparent mal à
/// l'œil, et compter des lignes sur l'un pour les retrouver sur l'autre est
/// exactement ce qu'un plan existe pour éviter.
fn mark(
    output: &mut Output<'_>,
    maze: &Grid,
    cell: (u32, u32, u32),
    here: u32,
    left: u32,
    top: u32,
) {
    let inner = CELL - 2;

    // **Départ et sortie en anneau, volées en plein**, et c'est la forme qui
    // porte la différence : à cinq pixels de côté, le vert du départ et la teinte
    // d'une volée verte ne se distinguent pas, et on cherche un escalier là où
    // commence la partie.
    if cell == maze.start() {
        ring(output, left + 1, top + 1, inner, START);
    }
    if cell == maze.exit() {
        ring(output, left + 1, top + 1, inner, EXIT);
    }
    // **La cellule qui couvre la case, et non l'identifiant de la case.** Une cage
    // tient deux cases superposées et sort au rang de son pied : celle du haut
    // porte un identifiant qu'aucune cellule du fichier ne nomme, et se comparer
    // à lui ne marquait jamais l'étage où l'on se trouve vraiment.
    //
    // Les deux cases d'une cage s'allument donc ensemble, ce qui est exact — on
    // est dans une cellule qui les occupe toutes les deux — et commode : la cage
    // se repère du même coup sur les deux plans.
    if export::cover(maze, cell) == here {
        block(output, left + 2, top + 2, inner - 2, inner - 2, HERE);
        return;
    }
    if cell == maze.start() || cell == maze.exit() {
        return;
    }

    // Une volée tient deux cases superposées, et les deux portent sa teinte :
    // c'est ce qui permet de la suivre d'un plan à l'autre. La moitié haute dit
    // qu'on monte depuis cette case, la moitié basse qu'on y arrive.
    //
    // **Et la forme dit laquelle des deux sortes de cage**, parce que la teinte ne
    // peut pas : elle est prise par l'appariement, qui est la raison d'être du
    // marquage. Un escalier garde sa demi-case pleine ; une rampe reçoit un
    // triangle dont la pointe donne le sens de sa pente — vers la montée au pied,
    // vers la descente à la tête, donc deux pointes opposées qui distinguent les
    // deux cases comme les demi-cases le faisaient.
    //
    // On ne cherche pas une rampe en visitant six cages quand on règle une cote à
    // l'écran, et c'est tout ce que ce plan existe pour éviter.
    let half = inner / 2;
    for (rank, stair) in maze.stairs().iter().enumerate() {
        let colour = LINKS[rank % LINKS.len()];
        let foot = cell == stair.foot;
        if !foot && cell != stair.head() {
            continue;
        }
        match stair.shape {
            Shape::Ramp => {
                let towards = if foot {
                    stair.climb
                } else {
                    stair.climb.facing()
                };
                wedge(output, left + 1, top + 1, inner, towards, colour);
            }
            Shape::Steps if foot => block(output, left + 1, top + 1, inner, half, colour),
            Shape::Steps => block(
                output,
                left + 1,
                top + 1 + inner - half,
                inner,
                half,
                colour,
            ),
        }
    }
}

/// Un triangle plein pointant vers un côté, inscrit dans un carré.
///
/// **Le nord est en haut de l'écran**, comme pour le reste du plan, donc la pointe
/// d'une montée vers le nord va vers les ordonnées décroissantes du tampon.
///
/// **La base occupe le côté opposé à celui vers lequel la pointe va**, et c'est
/// la confusion qui a fait pointer les quatre bras à l'envers : le rang 0 écrit la
/// base, donc son bord est celui dont on s'éloigne.
///
/// La base fait tout le côté et le triangle en occupe un peu plus de la moitié :
/// à cinq pixels, une pointe franche se lit mieux qu'un triangle qui remplirait le
/// carré en perdant son sommet dans les bords.
fn wedge(output: &mut Output<'_>, x: u32, y: u32, side: u32, towards: Side, color: [u8; 4]) {
    for rank in 0..side.div_ceil(2) {
        let span = side - 2 * rank;
        match towards {
            Side::East => block(output, x + rank, y + rank, 1, span, color),
            Side::West => block(output, x + side - 1 - rank, y + rank, 1, span, color),
            Side::North => block(output, x + rank, y + side - 1 - rank, span, 1, color),
            _ => block(output, x + rank, y + rank, span, 1, color),
        }
    }
}

/// Le fond du plan, derrière tout le reste.
const BACKDROP: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

/// La teinte d'un mur.
const WALL: [u8; 4] = [0x90, 0x94, 0xA4, 0xFF];

/// Celle de la case où l'on se trouve.
const HERE: [u8; 4] = [0xFF, 0xFF, 0xFF, 0xFF];

/// Celle de la case d'entrée.
const START: [u8; 4] = [0x40, 0xC0, 0x60, 0xFF];

/// Celle de la case de sortie.
const EXIT: [u8; 4] = [0xC8, 0x50, 0x40, 0xFF];

/// Les teintes des passages d'étage, prises à tour de rôle.
///
/// Douze, bien écartées sur le cercle : deux passages qui la partageraient sont
/// distants de douze rangs dans l'ordre de lecture, donc jamais voisins à
/// l'écran.
const LINKS: [[u8; 4]; 12] = [
    [0xE8, 0x7A, 0x5A, 0xFF],
    [0xE8, 0xA8, 0x40, 0xFF],
    [0xD8, 0xD8, 0x48, 0xFF],
    [0x9C, 0xD0, 0x48, 0xFF],
    [0x50, 0xC8, 0x90, 0xFF],
    [0x48, 0xD0, 0xD0, 0xFF],
    [0x58, 0xA8, 0xE8, 0xFF],
    [0x78, 0x80, 0xE8, 0xFF],
    [0xA8, 0x68, 0xE0, 0xFF],
    [0xE0, 0x70, 0xC0, 0xFF],
    [0xC8, 0x98, 0x78, 0xFF],
    [0xF0, 0xF0, 0xF0, 0xFF],
];

/// Un anneau carré d'un pixel d'épaisseur.
fn ring(output: &mut Output<'_>, x: u32, y: u32, side: u32, color: [u8; 4]) {
    block(output, x, y, side, 1, color);
    block(output, x, y + side - 1, side, 1, color);
    block(output, x, y, 1, side, color);
    block(output, x + side - 1, y, 1, side, color);
}

/// Un rectangle plein, borné par le tampon.
fn block(output: &mut Output<'_>, x: u32, y: u32, width: u32, height: u32, color: [u8; 4]) {
    for row in 0..height {
        for column in 0..width {
            if let Some(pixel) = output.pixel(x + column, y + row) {
                pixel.copy_from_slice(&color);
            }
        }
    }
}
