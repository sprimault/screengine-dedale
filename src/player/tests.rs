// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves du corps du joueur.
//!
//! Elles passent par le **chargement** et par le balayage, comme celles de
//! l'export : une pose ne se juge pas sur ses coordonnées mais sur ce que le
//! moteur en dit — dans le solide, ou posée sur quelque chose.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::maze::grid::{Settings, Side};
use screengine_play::{sweep_reach, sweep_skin};

/// Un labyrinthe d'épreuve, par sa graine.
///
/// **Plusieurs graines plutôt qu'une** : le départ est le centre de la grille et
/// les volées sont tirées, donc la forme du sol sous les pieds du joueur change
/// d'une graine à l'autre — plat, en marches, ou en rampe. Une seule graine
/// n'éprouverait qu'une des trois, et sans dire laquelle.
fn maze(seed: u64) -> (Grid, World) {
    let grid = Grid::generate(Settings {
        extent: (16, 16, 2),
        seed,
        stairs: 6,
        ramps: 2,
        loops: 8,
    });
    let map = World::load(&export::world(&grid)).expect("carte engendrée valide");
    (grid, map)
}

/// Les graines éprouvées, dont celle du jeu.
const SEEDS: [u64; 6] = [0x5EED_1A8E, 1, 2, 3, 0xD1CE, 0xFACE];

/// Les quatre côtés horizontaux, ceux qu'un pas de marche emprunte.
fn sides() -> Vec<Side> {
    Side::ALL.into_iter().filter(|s| !s.is_vertical()).collect()
}

/// Toutes les cases d'une grille, dans l'ordre des axes.
fn cases(grid: &Grid) -> Vec<(u32, u32, u32)> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::with_capacity((width * height * levels) as usize);
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                out.push((x, y, z));
            }
        }
    }
    out
}

/// La marge que le moteur laisse entre la boîte et ce qu'elle touche.
///
/// **Appelée et non recopiée** : le facteur porte sur la plus grande
/// demi-étendue et non sur chacune, et un nombre repris à la main se tromperait
/// sur exactement les boîtes où cette règle décide. C'est l'unité dans laquelle
/// s'énoncent les écarts tolérés ci-dessous — ce qui reste sous elle n'a pas
/// bougé.
fn skin() -> f32 {
    sweep_skin(HALF)
}

/// La direction unitaire d'un côté, en unités de monde.
fn unit(side: Side) -> Vec3 {
    let (dx, dy, dz) = side.step();
    Vec3::new(dx as f32, dy as f32, dz as f32)
}

/// Les deux côtés horizontaux perpendiculaires à celui-ci.
///
/// Ce sont les tangentes d'un mur : les directions dans lesquelles une glissade
/// peut emporter ce qui reste du pas.
fn across(side: Side) -> Vec<Side> {
    sides()
        .into_iter()
        .filter(|s| *s != side && *s != side.facing())
        .collect()
}

/// Les vingt-six directions d'un pas d'épreuve : les axes, et toutes leurs
/// combinaisons.
///
/// **Les diagonales comptent autant que les axes**, et plus encore : c'est un pas
/// oblique qui distingue une glissade d'un arrêt, et c'est lui qui présente deux
/// normales à la fois dans un coin.
///
/// **Le vertical en fait partie, et ce n'est pas une anticipation de la gravité** :
/// les deux touches du vol d'inspection montent et descendent aujourd'hui, donc un
/// pas qui rencontre un sol ou un plafond **en même temps** qu'un mur existe déjà.
/// C'est là que trois plans se présentent dans le même pas.
fn bearings() -> Vec<Vec3> {
    let mut out = Vec::with_capacity(26);
    for dx in [-1.0, 0.0, 1.0] {
        for dy in [-1.0, 0.0, 1.0] {
            for dz in [-1.0, 0.0, 1.0] {
                if dx != 0.0 || dy != 0.0 || dz != 0.0 {
                    out.push(Vec3::new(dx, dy, dz));
                }
            }
        }
    }
    out
}

/// Vrai si cette case a un sol plat, donc si aucune cage ne l'occupe.
///
/// **Comparer la cellule à l'identifiant de la case ne suffit pas** : pour le
/// **pied** d'une cage, les deux sont égaux — c'est elle qui donne son rang à la
/// cellule —, et seule la tête en diffère. Il faut donc interroger les volées.
///
/// C'est ce que veulent les deux prédicats de pas : le sol d'une cage monte, donc
/// un pas horizontal y rencontre une marche ou une rampe, ce qui est juste et
/// n'éprouve ni un mur ni un passage.
fn plain(grid: &Grid, at: (u32, u32, u32)) -> bool {
    !grid
        .stairs()
        .iter()
        .any(|stair| stair.foot == at || stair.head() == at)
}

/// Le joueur naît debout, jamais dans le solide.
///
/// **C'est l'épreuve que ce lot existe pour rendre.** Le départ est le centre de
/// la grille et rien n'interdit qu'il tombe sur une cage : l'empreinte du corps
/// y dépasse le palier et surplombe des marches, ou repose sur une rampe à
/// quarante-cinq degrés. Posé à la seule cote du sol, le corps les pénètre.
///
/// **Elle a attendu un correctif du moteur, et c'est elle qui l'a rendu visible
/// chez nous** : une cellule de case sur trois n'arrêtait rien, donc le balayage
/// qui pose le corps n'y rencontrait pas son sol et le laissait en dessous.
/// `un_mur_plein_arrete_un_pas` porte la mesure qui a désigné la cause.
#[test]
fn le_joueur_nait_hors_du_solide() {
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let player = Player::spawn(&grid, &map);
        let cell = player.cell();
        assert_ne!(cell, 0, "graine {seed:#x} : le joueur naît hors du décor");

        // Un pas nul ne rencontre rien, par contrat : il faut un mouvement pour
        // que le moteur se prononce, et le plus court qui dise quelque chose est
        // une descente — c'est aussi celle qui porte le poids du corps.
        let below = Vec3::new(player.centre.x, player.centre.y, player.centre.z - 1.0);
        let hit = map
            .sweep(cell, HALF, player.centre, below)
            .expect("la cellule du joueur existe");
        assert!(
            !hit.start_solid,
            "graine {seed:#x} : le joueur naît dans le solide en {:?}, \
             contre la surface {} de normale {:?}",
            player.centre, hit.surface, hit.normal
        );
    }
}

/// Et il naît **posé** : ce qu'il a sous les pieds l'arrête aussitôt.
///
/// La distinction avec l'épreuve précédente est celle qui sépare un personnage
/// debout d'un personnage qui flotte : la première dit qu'il ne pénètre rien, et
/// seule celle-ci dit qu'il touche. Une pose relevée d'un centimètre de trop les
/// passerait toutes les deux et ferait tomber le joueur à la première image de
/// gravité.
///
/// **Arrêté dans la bande de contact, et non à la fraction zéro exacte.** Le
/// moteur rend une position *juste avant* ce qu'elle touche, donc une sonde reprise
/// de là descend d'un résidu avant de se prononcer — mesuré à deux
/// cent-millièmièmes de millimètre. Ce qui se vérifie est donc la course, qui doit
/// rester sous la dilatation de la boîte : un millième de sa plus grande
/// demi-étendue.
///
/// Elle a attendu le même correctif que l'épreuve précédente, pour la même raison.
#[test]
fn le_joueur_nait_pose_sur_son_sol() {
    /// La course de la sonde, en unités de monde.
    const PROBE: f32 = 1.0;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let player = Player::spawn(&grid, &map);

        let below = Vec3::new(player.centre.x, player.centre.y, player.centre.z - PROBE);
        let hit = map
            .sweep(player.cell(), HALF, player.centre, below)
            .expect("la cellule du joueur existe");
        assert!(
            hit.fraction * PROBE <= HALF.z / 1024.0,
            "graine {seed:#x} : le joueur descend de {} avant de toucher, depuis {:?}",
            hit.fraction * PROBE,
            player.centre
        );
        // Le sol, et non un flanc : une normale qui regarde le haut. Plat, en
        // marches ou en rampe, sa composante verticale reste la plus grande.
        assert!(
            hit.normal.z > 0.5,
            "graine {seed:#x} : ce qui le porte a pour normale {:?}",
            hit.normal
        );
    }
}

/// L'œil est au-dessus du centre du corps, et d'une hauteur plausible.
///
/// **Ce n'est pas une paraphrase du calcul** : elle fige le sens de la
/// dérivation, qui est tout ce que l'étape impose — un volume centré sur l'œil
/// flotterait au-dessus de ce qui est bas et le traverserait. Elle attraperait un
/// signe inversé, que rien d'autre ne dirait avant l'écran.
#[test]
fn l_oeil_est_au_dessus_du_corps() {
    let (grid, map) = maze(SEEDS[0]);
    let player = Player::spawn(&grid, &map);
    let eye = player.eye();

    assert_eq!(eye.x, player.centre.x);
    assert_eq!(eye.y, player.centre.y);
    assert_eq!(eye.z, player.centre.z + EYE_ABOVE);
    assert!(
        eye.z - (player.centre.z - HALF.z) > 1.0,
        "l'œil est à {} du sol, ce qui n'est pas une taille d'homme",
        eye.z - (player.centre.z - HALF.z)
    );
}

/// Le joueur ne franchit aucun mur plein, quelle que soit la direction.
///
/// **C'est le prédicat du lot**, et il porte sur la boucle et non sur la boîte :
/// `un_mur_plein_arrete_un_pas` éprouve déjà `sweep` sur un volume nu, celui-ci
/// éprouve ce que le joueur en fait — la cellule qu'il passe, la fraction qu'il
/// applique, et le suivi qui vient derrière.
///
/// Le pas vaut une case entière, soit deux fois et demie ce qui sépare le corps du
/// mur : un arrêt ne peut donc pas être un hasard d'arrondi, et la distance rendue
/// dit de combien il a vraiment avancé.
#[test]
fn le_joueur_ne_franchit_pas_un_mur() {
    let (grid, map) = maze(SEEDS[0]);

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if !grid.has_wall(at, side) {
                continue;
            }
            let mut player = Player::stand(&grid, &map, at);
            let before = player.centre;
            let (dx, dy, _) = side.step();
            let step = Vec3::new(dx as f32 * export::CELL, dy as f32 * export::CELL, 0.0);

            let travel = player.advance(&map, step);
            let gone = player.centre - before;

            assert!(
                gone.dot(gone).sqrt() < export::CELL,
                "le joueur franchit le mur {side:?} de {at:?}, il a parcouru {}",
                gone.dot(gone).sqrt()
            );
            assert_eq!(
                travel,
                gone.dot(gone).sqrt(),
                "la distance rendue n'est pas celle parcourue"
            );
        }
    }
}

/// Un pas dans un passage ouvert reste libre.
///
/// **Le filtrage ne doit pas inventer d'obstacle**, et c'est l'autre moitié du
/// prédicat : un contrôle qui arrête tout passerait l'épreuve précédente. Le pas
/// va d'un point sûr au point sûr voisin, comme les épreuves de l'export.
#[test]
fn un_passage_ouvert_laisse_passer_le_joueur() {
    let (grid, map) = maze(SEEDS[0]);

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            if !plain(&grid, next) {
                continue;
            }

            let mut player = Player::stand(&grid, &map, at);
            let target = Player::stand(&grid, &map, next).centre;
            let travel = player.advance(&map, target - player.centre);

            assert!(
                travel > 0.0,
                "le joueur ne passe pas de {at:?} vers {next:?}"
            );
        }
    }
}

/// Un pas oblique contre un mur avance le long de ce mur.
///
/// **C'est le prédicat du lot**, et c'est lui qui distingue une glissade d'un
/// arrêt : sans projection, le pas s'arrêterait net au contact et la part
/// tangentielle serait perdue — ici le tiers de ce qui était demandé, puisque le
/// mur est à moins d'un tiers de case.
///
/// Le pas vise le mur d'une case entière, bien au-delà de ce qui sépare le corps
/// de la paroi, et porte en travers un demi-mètre : assez court pour rester dans
/// la cellule, donc rien d'autre que le mur ne peut l'arrêter, et c'est ce qui
/// rend l'écart toléré aussi serré que la marge du moteur.
#[test]
fn un_pas_oblique_le_long_d_un_mur_glisse() {
    /// Ce que le pas porte en travers du mur, en unités de monde.
    const ALONG: f32 = 0.5;

    let (grid, map) = maze(SEEDS[0]);
    let mut seen = 0;

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if !grid.has_wall(at, side) {
                continue;
            }
            for tangent in across(side) {
                let mut player = Player::stand(&grid, &map, at);
                let before = player.centre;
                let aim = unit(side) * export::CELL + unit(tangent) * ALONG;

                player.advance(&map, aim);
                let kept = (player.centre - before).dot(unit(tangent));

                assert!(
                    kept >= ALONG - skin(),
                    "contre le mur {side:?} de {at:?}, le pas vers {tangent:?} \
                     n'a gardé que {kept} de {ALONG}"
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "aucun mur éprouvé : la graine n'en porte pas");
}

/// Un pas de face contre un mur avance jusqu'à lui, sans dériver sur le côté.
///
/// **L'autre moitié du prédicat** : un pas perpendiculaire n'a rien à glisser, et
/// la glissade ne doit donc rien y changer — le reste du déplacement est
/// entièrement dans la normale, et la projection le réduit à rien.
///
/// **C'est l'avancée qui rend l'épreuve falsifiable, pas l'absence de dérive.**
/// Celle-ci ne peut pas arriver : le balayage refuse tout reste qui entre dans le
/// mur, si bien qu'une normale faussée de quarante-cinq degrés passe l'épreuve —
/// mesuré. Une politique de rebond, elle, ferait reculer au lieu d'avancer, et
/// c'est ce que la première assertion attrape.
#[test]
fn un_pas_de_face_contre_un_mur_ne_derive_pas() {
    let (grid, map) = maze(SEEDS[0]);

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if !grid.has_wall(at, side) {
                continue;
            }
            let mut player = Player::stand(&grid, &map, at);
            let before = player.centre;

            player.advance(&map, unit(side) * export::CELL);
            let gone = player.centre - before;

            assert!(
                gone.dot(unit(side)) > 0.0,
                "le pas de face contre le mur {side:?} de {at:?} n'a pas atteint \
                 la paroi : il a fait {}",
                gone.dot(unit(side))
            );
            for tangent in across(side) {
                let drift = gone.dot(unit(tangent));
                assert!(
                    drift.abs() <= skin(),
                    "le pas de face contre le mur {side:?} de {at:?} a dérivé \
                     de {drift} vers {tangent:?}"
                );
            }
        }
    }
}

/// Un coude se prend : poussé dans l'angle, le corps sort par l'ouverture.
///
/// **Le coude est un mur devant et un passage sur le côté**, et non deux murs :
/// un angle fermé arrête au coin avec ou sans glissade — les deux parois étant à
/// la même distance d'un corps centré, le balayage s'y arrête de lui-même, et une
/// épreuve bâtie sur ce cas ne mesurerait que le balayage. C'est ce qui l'a fait
/// réécrire : elle passait sur un code privé de sa projection.
///
/// **L'attendu est la progression dans l'ouverture** : le pas pousse d'une case
/// vers le mur et d'une case vers le passage, et le mur l'arrête à moins d'un
/// tiers de case. Seul ce qui reste du pas, reporté le long du mur, emmène le
/// corps au-delà du portail — sans glissade il s'arrête au coin, bien en deçà de
/// la demi-case exigée ici.
///
/// **Et non la cellule d'arrivée, qui serait fausse** : le mur du coude n'existe
/// pas forcément dans la case suivante, si bien que le reste du pas y repart en
/// diagonale et dépasse la voisine immédiate. C'est le pas demandé qui s'accomplit,
/// et l'épreuve ne doit pas le prendre pour un défaut.
#[test]
fn un_coude_ne_bloque_pas() {
    let (grid, map) = maze(SEEDS[0]);
    let mut seen = 0;

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if !grid.has_wall(at, side) {
                continue;
            }
            for tangent in across(side) {
                if grid.has_wall(at, tangent) {
                    continue;
                }
                let next = grid
                    .neighbour(at, tangent)
                    .expect("un passage a une voisine");
                if !plain(&grid, next) {
                    continue;
                }

                let mut player = Player::stand(&grid, &map, at);
                let before = player.centre;
                let aim = (unit(side) + unit(tangent)) * export::CELL;

                player.advance(&map, aim);
                let taken = (player.centre - before).dot(unit(tangent));

                assert!(
                    taken >= export::CELL / 2.0,
                    "poussé dans le coude {side:?}/{tangent:?} de {at:?}, le corps \
                     n'a pris que {taken} vers {next:?}"
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "aucun coude éprouvé : la graine n'en porte pas");
}

/// Aucune glissade ne fait reculer, où qu'on pousse.
///
/// **C'est la garde de la boucle**, et elle vaut d'être éprouvée partout : deux
/// projections successives peuvent renverser ce qui reste du pas, et un joueur
/// qui part en arrière alors qu'il pousse vers l'avant est ce qui se voit le plus
/// vite à l'écran. Un déplacement de composante négative sur le pas demandé est
/// donc un défaut, quelle que soit la géométrie rencontrée.
#[test]
fn la_glissade_ne_fait_jamais_reculer() {
    let (grid, map) = maze(SEEDS[0]);

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for bearing in bearings() {
            let aim = bearing * export::CELL;
            let mut player = Player::stand(&grid, &map, at);
            let before = player.centre;

            player.advance(&map, aim);
            let gone = player.centre - before;

            assert!(
                gone.dot(aim) >= 0.0,
                "en {at:?}, un pas vers {aim:?} a reculé de {gone:?}"
            );
        }
    }
}

/// Longer un mur pas à pas avance sans accrocher.
///
/// **Les autres épreuves partent toutes du centre d'une case**, donc d'un corps
/// loin de toute paroi, et aucune ne voit ce qui arrive après la première image :
/// en jouant, on longe un mur **collé** à lui, et chaque pas repart d'un contact.
/// C'est l'angle mort qui a laissé passer un blocage visible à l'écran.
///
/// Le pas vaut ce qu'une image parcourt, et non une case : c'est le régime réel,
/// et c'est lui qui enchaîne les contacts.
///
/// **Elle est née rouge et le moteur l'a rendue verte.** Un corps posé à la
/// distance de contact que le balayage rend lui-même était arrêté par l'**arête
/// terminale** du panneau qu'il longe : fraction `1,67e-6` au lieu de 1, normale
/// perpendiculaire à celle du mur, et la même surface nommée pour les deux. Le
/// bord d'un panneau arrête désormais une demi-marge plus tard que sa face, ce qui
/// laisse au contact la place de repartir tangent.
///
/// **La case d'arrivée doit être plate elle aussi**, et pas seulement celle de
/// départ : une cage présente une contremarche en travers du chemin, que la
/// glissade ne franchit pas et n'a pas à franchir — monter ce qui se monte est
/// l'affaire du seuil, qui n'existe pas encore.
#[test]
fn longer_un_mur_pas_a_pas_avance() {
    /// Ce qu'une image parcourt, en unités de monde.
    const STEP: f32 = 0.05;
    /// Combien d'images la sonde enchaîne.
    const FRAMES: usize = 120;

    let (grid, map) = maze(SEEDS[0]);
    let mut seen = 0;

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if !grid.has_wall(at, side) {
                continue;
            }
            for tangent in across(side) {
                if grid.has_wall(at, tangent) {
                    continue;
                }
                let beyond = grid.neighbour(at, tangent);
                if !beyond.is_some_and(|case| plain(&grid, case)) {
                    continue;
                }

                let mut player = Player::stand(&grid, &map, at);
                let before = player.centre;
                let push = (unit(side) + unit(tangent)) * STEP;
                for _ in 0..FRAMES {
                    player.advance(&map, push);
                }
                let taken = (player.centre - before).dot(unit(tangent));

                // La moitié de ce qui a été demandé en travers : large, parce que
                // la case voisine peut fermer plus loin. Ce qui est cherché est un
                // blocage, pas une mesure fine.
                let asked = STEP * FRAMES as f32;
                assert!(
                    taken >= asked / 2.0,
                    "le long du mur {side:?} de {at:?} vers {tangent:?}, \
                     {FRAMES} pas n'ont avancé que de {taken} sur {asked}"
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "aucun mur longeable éprouvé");
}

/// La glissade garde de la marge sur le nombre de plans qu'elle peut consommer.
///
/// **C'est ce qui fait de [`SLIDES`] une mesure et non un ordre de grandeur.** Le
/// raisonnement disait trois plans au pire — deux murs et un sol, ou une rampe et
/// deux murs ; cette épreuve relève ce qui est réellement employé sur toutes les
/// cases et les huit directions, et exige qu'il reste **sous** la borne. Si elle
/// échoue, c'est le raisonnement qu'il faut reprendre, pas la constante qu'il faut
/// relever.
#[test]
fn une_glissade_garde_sa_marge() {
    let (grid, map) = maze(SEEDS[0]);
    let mut worst = 0;
    let mut whence = None;

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for bearing in bearings() {
            let player = Player::stand(&grid, &map, at);
            let wanted = player.centre + bearing * export::CELL;
            let (_, planes) = player.slide(&map, wanted);

            if planes > worst {
                worst = planes;
                whence = Some((at, bearing));
            }
        }
    }

    assert!(
        worst < SLIDES,
        "une glissade a consommé les {SLIDES} plans de la borne, en {whence:?}"
    );
}

/// Le décor joué tient là où la boîte du joueur garde son jeu de collision.
///
/// **Une épreuve de dimensionnement, et elle est verte d'avance** : le balayage
/// laisse à la boîte un jeu qui se perd avec l'éloignement de l'origine, et une
/// grille de seize cases est à deux ordres de grandeur en dessous. Ce qu'elle garde
/// est donc l'avenir — une grille plus large, ou une boîte plus petite qu'on
/// balaierait —, et c'est pourquoi elle part du réglage **joué** et non de celui des
/// épreuves : c'est lui qu'on agrandit.
///
/// **La distance s'appelle et ne se recopie pas**, comme la marge : elle dérive
/// d'une constante qui ne fait pas partie du contrat, et un seuil recopié se
/// tromperait sur exactement les boîtes où il décide.
#[test]
fn le_decor_joue_garde_le_jeu_de_la_boite() {
    let (width, height, levels) = crate::MAZE.extent;

    // L'export pose la grille depuis l'origine, donc la plus grande coordonnée du
    // décor est celle du coin opposé — en hauteur, le plafond du dernier étage.
    let far = (width as f32 * export::CELL)
        .max(height as f32 * export::CELL)
        .max((levels - 1) as f32 * export::LEVEL + export::CEILING);

    // Strictement : à cette distance le jeu est déjà perdu, c'est le seuil et non
    // la dernière cote sûre.
    assert!(
        far < sweep_reach(HALF),
        "le décor va jusqu'à {far}, et la boîte du joueur perd son jeu à {}",
        sweep_reach(HALF)
    );
}
