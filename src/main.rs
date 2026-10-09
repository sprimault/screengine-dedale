// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le point d'entrée du jeu : les réglages, la boucle, et les trois rappels.
//!
//! **Les rappels ne portent rien.** Le pas de mise à jour délègue à l'état de la
//! partie, le rendu à `scene::submit`, et la sortie au plan de contrôle. C'est
//! voulu : la boucle ouvre une fenêtre, donc elle ne servira pas à l'image animée
//! du `README`, et tout ce qui vivrait dans un rappel serait à en sortir ce
//! jour-là.

mod blot;
mod body;
mod game;
mod hud;
mod mark;
mod maze;
mod monster;
mod player;
mod probe;
mod scene;
mod sheet;
mod shot;
mod weapon;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

use game::{Game, Run};
use hud::block;
use hud::glyph::{self, Glyphs};
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
/// la traversée est jetée avec lui, et rien de l'une n'entre dans l'autre.
///
/// **Et la partie se tient en deux morceaux**, depuis que la vie existe : la
/// traversée d'une carte, qui se refait au labyrinthe suivant, et la course, qui le
/// franchit. Les ranger ensemble remettrait le cumul au plein à chaque niveau.
struct Session {
    /// L'état du monde.
    scenery: Scenery,
    /// La traversée de la carte chargée.
    game: Game,
    /// Ce que la course garde d'un labyrinthe au suivant.
    run: Run,
    /// Le relevé de ce que la traversée rend.
    ///
    /// **Ni du monde ni de la partie** : c'est un instrument, qui ne décide de
    /// rien et que le rendu seul alimente. Le ranger dans l'un des deux ferait
    /// passer un journal pour un état de jeu.
    probe: Probe,
    /// La planche de glyphes, chargée une fois.
    ///
    /// Ni du monde ni de la partie non plus : une ressource, que rien ne modifie
    /// et dont la relance n'a aucune raison de refaire le décodage.
    glyphs: Glyphs,
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
            run: Run::new(),
            probe: Probe::new(),
            glyphs: Glyphs::new(),
        },
        |session, tick| {
            if tick.input().pressed(KeyCode::Escape) {
                tick.exit();
            }

            // **La relance est une course neuve sur la même carte** : le monde ne
            // bouge pas, donc seules la traversée et la course se refont — la
            // population comprise, ce qui est la sémantique d'une partie qui
            // recommence et non d'une résurrection sur place.
            //
            // **Un échec ne peut venir que d'une planche intégrée au binaire**, donc
            // déjà décodée au lancement. Le rappel ne sait pas porter une erreur :
            // fermer est la seule réponse franche, et paniquer sur un chemin que rien
            // ne rend atteignable n'en est pas une.
            if session.run.over() && tick.input().pressed(KeyCode::KeyR) {
                match Game::new(&session.scenery.maze, &session.scenery.map) {
                    Ok(game) => {
                        session.game = game;
                        session.run = Run::new();
                    }
                    Err(_) => tick.exit(),
                }
            }

            if let Some(shot) = session
                .game
                .step(tick, &session.scenery.map, &mut session.run)
            {
                let aim = session.game.aim();
                session.probe.fired(&shot, &session.scenery.map, &aim);
            }

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
            let (figure, reach) = session.game.nearest();
            // Le titre ne dit plus la mort : elle s'écrit à l'écran, là où on la lit
            // en jouant. Ce qui reste ici est un relevé de développement, que
            // personne ne consulte en jouant.
            tick.set_title(&format!(
                "{title} — œil {:.2} sur {:.2} d'étage, plafond {:.2}, \
                 démon {figure} à {reach:.2}, cellule {}, vue {:?}",
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
            // **Les créatures après le décor, l'arme en dernier.** L'ordre ne tient
            // pas au mélange — la transparence du moteur est binaire, donc le
            // z-buffer tranche dans n'importe quel ordre et aucun tri n'est à faire
            // — mais à la profondeur : un décor très proche rejetterait à égalité ce
            // qui lui est collé, et l'arme est la plus proche de l'œil de toutes.
            //
            // Leur refus se relève comme celui du décor : il vient de la même
            // capacité, et une arme qui disparaît de la main ou un démon qui
            // s'efface d'un couloir sont des symptômes qu'on chercherait longtemps
            // sans la ligne qui le dit.
            // Les marques juste après le décor, et avant les créatures : une surface
            // modulée n'assombrit que ce qui est déjà au tampon, donc elle doit suivre
            // ce qu'elle marque. Elle n'a rien à voir avec les sprites, qui passent
            // après parce qu'ils sont plus proches de l'œil.
            let traces = mark::submit(context, session.game.marks());
            session
                .probe
                .refusal(probe::Part::Marks, traces.is_err(), &session.game.aim());

            let seen = monster::submit(context, session.game.monsters(), &view.camera);
            session
                .probe
                .refusal(probe::Part::Monsters, seen.is_err(), &session.game.aim());

            let hand = weapon::submit(context, session.game.weapon(), &view.camera);
            session
                .probe
                .refusal(probe::Part::Weapon, hand.is_err(), &session.game.aim());
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

/// Le rappel de sortie : le relevé de l'image finie, puis l'interface par-dessus.
///
/// **Dans cet ordre et pas l'autre.** Le relevé échantillonne deux cent cinquante-six
/// points du tampon et les compare au fond de la scène : tout ce qui se dessine avant
/// lui compte comme un pixel peint par la soumission, et l'instrument devient d'autant
/// aveugle à une image amputée.
///
/// **Rien ne tient cet ordre, et c'est à savoir en ajoutant à l'interface.** Il ne
/// s'éprouve pas d'ici — le rappel reçoit une session que rien ne fabrique sans ouvrir
/// le fichier de relevé —, donc ce qui est mesuré est sa conséquence :
/// `le_releve_voit_l_interface` vérifie que la jauge compte, et qu'elle reste seule
/// sous le seuil. Ce que le HUD recevra ensuite s'ajoute à ce compte.
fn overview(session: &mut Session, output: &mut Output<'_>) {
    // Un échantillon du tampon dit si l'image est amputée, ce qu'aucune valeur
    // rendue par la soumission ne dit. La couleur du fond vient de la scène, qui
    // seule sait ce qu'un pixel non peint porte.
    let aim = session.game.aim();
    session
        .probe
        .look(output, &session.scenery.map, &aim, scene::BACKGROUND);

    // Le réticule après le plan, et il ne le croise pas : l'un est au coin, l'autre au
    // centre. L'ordre ne tient donc qu'à ce que le réticule soit le dernier mot de
    // l'image, comme ce qu'on regarde en tirant.
    plot(output, &session.scenery.maze, aim.cell);
    hud::gauge::draw(output, session.run.share());
    hud::score::draw(output, &session.glyphs, session.run.score());
    if session.run.over() {
        epitaph(output, &session.glyphs);
    }
    hud::reticle::draw(output);
}

/// Les deux lignes de la mort, et l'échelle de chacune.
///
/// Deux tailles plutôt qu'une : l'état se lit d'un coup d'œil, la touche se lit
/// quand on la cherche. Réglées à l'écran, comme toute cote d'interface.
const EPITAPH: [(&str, u32); 2] = [("MORT", 4), ("R RELANCE", 2)];

/// L'écart entre les deux lignes, en pixels.
const LINE_GAP: u32 = 8;

/// Ce qui s'écrit quand la course est finie, au milieu de l'écran.
///
/// **Avant le réticule et après la jauge** : il reste le dernier mot de l'image, et
/// un texte posé par-dessus lui ferait croire qu'on vise encore.
///
/// Le centrage se prend du tampon à chaque image, comme le reste du HUD : la
/// résolution interne n'est pas celle de la fenêtre et peut changer en jouant.
///
/// **Il gonfle le compte du relevé, et c'est sans conséquence** : le relevé passe
/// avant lui et compte les points peints, donc ce texte ne peut que **masquer** une
/// image amputée, jamais en inventer une. Et il ne s'écrit que la course finie, c'est-à-dire
/// quand plus rien ne bouge et qu'il n'y a plus d'épisode à relever.
///
/// Le décalage de l'ombre vaut l'échelle, pour qu'une ligne deux fois plus grande
/// porte une ombre deux fois plus épaisse ; les deux teintes viennent du module du
/// texte, qui les partage avec le compteur.
fn epitaph(output: &mut Output<'_>, glyphs: &Glyphs) {
    let height: u32 = EPITAPH
        .iter()
        .map(|&(_, scale)| glyphs.height(scale))
        .sum::<u32>()
        + LINE_GAP;
    let mut top = output.height().saturating_sub(height) / 2;
    for (text, scale) in EPITAPH {
        let left = output.width().saturating_sub(glyphs.width(text, scale)) / 2;
        let shadow = (left + scale, top + scale);
        glyphs.draw(output, shadow, text, scale, glyph::SHADOW);
        glyphs.draw(output, (left, top), text, scale, glyph::INK);
        top += glyphs.height(scale) + LINE_GAP;
    }
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
