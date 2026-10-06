// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la politique de collision : glissade, chute, collage,
//! franchissement, dégagement.
//!
//! Elles passent par le **chargement** et par le balayage, comme celles de
//! l'export : une pose ne se juge pas sur ses coordonnées mais sur ce que le
//! moteur en dit — dans le solide, ou posée sur quelque chose.
//!
//! **Elles éprouvent le gabarit du joueur, et c'est voulu** : c'est celui dont le
//! domaine de validité est établi par les épreuves de l'export, donc le seul contre
//! lequel les écarts tolérés ci-dessous ont un sens. Un autre gabarit n'y est
//! éprouvé qu'une fois, par `un_autre_gabarit_emprunte_le_meme_chemin`, dont c'est
//! tout l'objet.
//!
//! **Les écarts tolérés s'énoncent en marges du balayage**, appelées et non
//! recopiées : le facteur porte sur la plus grande demi-étendue et non sur chacune,
//! et un nombre repris à la main se tromperait sur exactement les boîtes où cette
//! règle décide. Ce qui reste sous une marge n'a pas bougé.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::maze::grid::Shape;
use crate::monster;
use crate::player::HALF;
use crate::test_support::{DT, SEEDS, across, bearings, cases, maze, plain, sides, unit};
use screengine_play::sweep_reach;

/// Vrai si le corps est dans le solide, par une sonde que le moteur tranche.
///
/// **Un pas, et non une position** : le balayage ne se prononce que sur un
/// mouvement, donc la sonde descend. Sa longueur n'a pas d'importance — un départ
/// solide se signale avant qu'elle serve.
///
/// **Elle interroge le gabarit du corps**, ce qui la rend juste pour un autre que
/// celui du joueur : c'est exactement ce que l'épreuve d'un gabarit tiers demande.
fn solid(map: &World, body: &Body) -> bool {
    let below = Vec3::new(body.centre.x, body.centre.y, body.centre.z - 1.0);
    map.sweep(body.cell(), body.half, body.centre, below)
        .is_some_and(|hit| hit.start_solid)
}

/// Un corps posé à la seule cote du sol, au milieu d'un palier de cage.
///
/// **C'est le seul départ solide que ce décor produise**, et c'est pour cela que
/// les oracles de l'export l'écartent : l'empreinte dépasse le palier et surplombe
/// des marches, que `SLOPE` corrige dans [`Body::stand`]. On reprend donc la pose
/// **sans** cette correction, ce qui la met dans le solide à coup sûr.
fn stuck(grid: &Grid, at: (u32, u32, u32)) -> Body {
    let spot = export::ground(grid, at);
    let centre = Vec3::new(spot[0], spot[1], spot[2] + HALF.z);

    Body {
        half: HALF,
        centre,
        cell: export::cover(grid, at),
        previous: centre,
        fall: 0.0,
    }
}

/// Un corps suspendu au-dessus du sol d'une case, de quoi le faire tomber.
///
/// **La cote se prend du sol et non du plafond** : une case porte un étage de 3,5
/// sous un plafond de 3,25, donc un corps relevé d'un mètre a encore sa tête sous la
/// dalle — ce qu'une pose aveugle n'aurait pas.
fn aloft(grid: &Grid, at: (u32, u32, u32), lift: f32) -> Body {
    let spot = export::ground(grid, at);
    let centre = Vec3::new(spot[0], spot[1], spot[2] + HALF.z + lift);

    Body {
        half: HALF,
        centre,
        cell: export::cover(grid, at),
        previous: centre,
        fall: 0.0,
    }
}

/// Un corps ne franchit aucun mur plein, quelle que soit la direction.
///
/// **C'est le prédicat du lot**, et il porte sur la boucle et non sur la boîte :
/// `un_mur_plein_arrete_un_pas` éprouve déjà `sweep` sur un volume nu, celui-ci
/// éprouve ce que le corps en fait — la cellule qu'il passe, la fraction qu'il
/// applique, et le suivi qui vient derrière.
///
/// Le pas vaut une case entière, soit deux fois et demie ce qui sépare le corps du
/// mur : un arrêt ne peut donc pas être un hasard d'arrondi, et la distance rendue
/// dit de combien il a vraiment avancé.
#[test]
fn un_corps_ne_franchit_pas_un_mur() {
    let (grid, map) = maze(SEEDS[0]);

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for side in sides() {
            if !grid.has_wall(at, side) {
                continue;
            }
            let mut body = Body::stand(HALF, &grid, &map, at);
            let before = body.centre;
            let (dx, dy, _) = side.step();
            let step = Vec3::new(dx as f32 * export::CELL, dy as f32 * export::CELL, 0.0);

            let travel = body.advance(&map, step, DT);
            let gone = body.centre - before;

            assert!(
                gone.dot(gone).sqrt() < export::CELL,
                "le corps franchit le mur {side:?} de {at:?}, il a parcouru {}",
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
fn un_passage_ouvert_laisse_passer_un_corps() {
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

            let mut body = Body::stand(HALF, &grid, &map, at);
            let target = Body::stand(HALF, &grid, &map, next).centre;
            let travel = body.advance(&map, target - body.centre, DT);

            assert!(
                travel > 0.0,
                "le corps ne passe pas de {at:?} vers {next:?}"
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
                let mut body = Body::stand(HALF, &grid, &map, at);
                let before = body.centre;
                let aim = unit(side) * export::CELL + unit(tangent) * ALONG;

                body.advance(&map, aim, DT);
                let kept = (body.centre - before).dot(unit(tangent));

                assert!(
                    kept >= ALONG - sweep_skin(HALF),
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
            let mut body = Body::stand(HALF, &grid, &map, at);
            let before = body.centre;

            body.advance(&map, unit(side) * export::CELL, DT);
            let gone = body.centre - before;

            assert!(
                gone.dot(unit(side)) > 0.0,
                "le pas de face contre le mur {side:?} de {at:?} n'a pas atteint \
                 la paroi : il a fait {}",
                gone.dot(unit(side))
            );
            for tangent in across(side) {
                let drift = gone.dot(unit(tangent));
                assert!(
                    drift.abs() <= sweep_skin(HALF),
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

                let mut body = Body::stand(HALF, &grid, &map, at);
                let before = body.centre;
                let aim = (unit(side) + unit(tangent)) * export::CELL;

                body.advance(&map, aim, DT);
                let taken = (body.centre - before).dot(unit(tangent));

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
/// projections successives peuvent renverser ce qui reste du pas, et un corps
/// qui part en arrière alors qu'il pousse vers l'avant est ce qui se voit le plus
/// vite à l'écran. Un déplacement de composante négative sur le pas demandé est
/// donc un défaut, quelle que soit la géométrie rencontrée.
///
/// **À une demi-marge près, et c'est la marge elle-même** : la glissade repart hors
/// de la bande de contact, donc un pas qui ne rencontre que le sol s'en écarte
/// d'autant — relevé à `4,4e-4` sur un pas vertical, soit exactement cet écart. Ce
/// que l'épreuve refuse est un recul, pas la marge qui rend le pas suivant possible.
#[test]
fn la_glissade_ne_fait_jamais_reculer() {
    let (grid, map) = maze(SEEDS[0]);

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for bearing in bearings() {
            let aim = bearing * export::CELL;
            let mut body = Body::stand(HALF, &grid, &map, at);
            let before = body.centre;

            body.advance(&map, aim, DT);
            let gone = body.centre - before;

            assert!(
                gone.dot(aim) >= -sweep_skin(HALF) * aim.dot(aim).sqrt(),
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

                let mut body = Body::stand(HALF, &grid, &map, at);
                let before = body.centre;
                let push = (unit(side) + unit(tangent)) * STEP;
                for _ in 0..FRAMES {
                    body.advance(&map, push, DT);
                }
                let taken = (body.centre - before).dot(unit(tangent));

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
            let body = Body::stand(HALF, &grid, &map, at);
            let wanted = body.centre + bearing * export::CELL;
            let planes = body.slide(&map, body.centre, wanted).planes;

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

/// Un départ dans le solide se dégage, en quelques images.
///
/// **L'épreuve part d'un état que les autres évitent**, et c'est tout son objet :
/// [`Body::stand`] corrige la pente du palier précisément pour ne jamais naître
/// ainsi, donc rien d'autre ici ne visite ce cas.
///
/// **Ce qu'elle exige est la sortie, pas un déplacement** : la pose d'arrivée reste
/// dans une cellule connue, et n'est plus solide. Dans cet ordre, parce que le
/// second critère est vide sans le premier — hors du décor, le balayage ne répond
/// plus rien et « pas solide » ne veut plus rien dire. C'est ce qu'elle a mesuré en
/// naissant rouge : laissé passer, le pas traversait les murs.
#[test]
fn un_depart_dans_le_solide_se_degage() {
    /// Ce qu'une image parcourt, en unités de monde.
    const STEP: f32 = 0.05;
    /// Combien d'images le dégagement a pour sortir.
    const FRAMES: usize = 60;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let mut seen = 0;

        for stair in grid.stairs() {
            let mut body = stuck(&grid, stair.foot);
            if !solid(&map, &body) {
                continue;
            }
            seen += 1;

            let from = body.centre;
            for _ in 0..FRAMES {
                body.advance(&map, Vec3::new(STEP, 0.0, 0.0), DT);
            }

            assert_ne!(
                body.cell(),
                0,
                "graine {seed:#x} : parti du palier de {:?} en {from:?}, \
                 le corps a quitté le décor",
                stair.foot
            );
            assert!(
                !solid(&map, &body),
                "graine {seed:#x} : parti du palier de {:?} en {from:?}, \
                 le corps est encore dans le solide en {:?}",
                stair.foot,
                body.centre
            );
        }

        assert!(seen > 0, "graine {seed:#x} : aucun départ solide produit");
    }
}

/// Une chute s'arrête sur le sol, et le corps y repose.
///
/// **Deux exigences et non une** : s'arrêter ne suffit pas — un corps arrêté *dans*
/// le plancher s'est arrêté aussi. C'est [`Body::grounded`] qui tranche, et il
/// refuse un départ pénétrant.
///
/// **Le relevé est d'un mètre**, assez pour que la pesanteur ait le temps
/// d'accélérer et que le pas d'une image ne couvre pas la distance d'un coup : la
/// chute se fait donc en plusieurs balayages, comme en jouant.
#[test]
fn une_chute_s_arrete_sur_le_sol() {
    /// La hauteur du lâcher, en unités de monde.
    const LIFT: f32 = 1.0;
    /// Combien d'images la chute a pour se faire.
    const FRAMES: usize = 120;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let mut seen = 0;

        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let mut body = aloft(&grid, at, LIFT);
            if body.grounded(&map) {
                continue;
            }
            seen += 1;

            let from = body.centre;
            for _ in 0..FRAMES {
                body.advance(&map, Vec3::new(0.0, 0.0, 0.0), DT);
            }

            assert!(
                body.grounded(&map),
                "graine {seed:#x} : lâché en {from:?} au-dessus de {at:?}, \
                 le corps ne repose sur rien en {:?}",
                body.centre
            );
        }

        assert!(seen > 0, "graine {seed:#x} : aucun lâcher éprouvé");
    }
}

/// Une chute courte tombe, elle ne colle pas au sol.
///
/// **C'est la garde du collage qui est en jeu** : le corps n'est reposé sur son sol
/// qu'au terme d'un pas **parti du contact**. Sans elle, le collage s'appliquerait
/// aussi en l'air, et un corps lâché à moins d'une marche du sol y serait porté d'un
/// coup — une chute courte cesserait d'être une chute.
///
/// Le lâcher se choisit entre les deux cotes qui décident : plus haut que la sonde de
/// contact, pour que le corps parte bien en l'air, et plus bas que la portée du
/// collage, sans quoi rien ne pourrait l'attirer et l'épreuve passerait à vide.
#[test]
fn une_chute_courte_n_est_pas_un_collage() {
    /// Le lâcher, en unités de monde.
    const LIFT: f32 = RISE * 0.8;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let mut seen = 0;

        for at in cases(&grid) {
            if !plain(&grid, at) {
                continue;
            }
            let mut body = aloft(&grid, at, LIFT);
            if body.grounded(&map) {
                continue;
            }
            seen += 1;

            let from = body.centre;
            body.advance(&map, Vec3::new(0.0, 0.0, 0.0), DT);

            assert!(
                !body.grounded(&map),
                "graine {seed:#x} : lâché en {from:?} au-dessus de {at:?}, le corps \
                 repose déjà en {:?} après une seule image",
                body.centre
            );
            assert!(
                body.centre.z < from.z,
                "graine {seed:#x} : lâché en {from:?}, le corps n'est pas tombé"
            );
        }

        assert!(seen > 0, "graine {seed:#x} : aucun lâcher éprouvé");
    }
}

/// Le critère de sol suit la plus forte pente du décor.
///
/// **Ce qu'il garde est l'accord entre deux endroits**, et c'est tout son objet : le
/// seuil se dérive de `SLOPE`, que l'export publie comme la plus raide qu'il
/// produise. Une pente qui s'ajouterait au décor sans que le critère suive se verrait
/// ici, et nulle part ailleurs — aucune carte engendrée ne porte par construction de
/// surface qui le dépasse, donc aucune épreuve de déplacement ne peut l'atteindre.
///
/// **Le seuil s'encadre plutôt qu'il ne s'égale** : à la pente exacte, le produit
/// vaut un aux arrondis près, et une épreuve posée sur cette égalité dirait le hasard
/// de l'arrondi plutôt que le critère.
#[test]
fn le_critere_de_sol_suit_la_pente_du_decor() {
    /// La normale, dirigée vers le haut, d'un plan montant de `slope` par unité.
    fn facing(slope: f32) -> Vec3 {
        let length = (1.0 + slope * slope).sqrt();
        Vec3::new(-slope / length, 0.0, 1.0 / length)
    }

    assert!(walkable(facing(0.0)), "un sol plat se marche");
    assert!(
        walkable(facing(SLOPE * 0.99)),
        "la plus forte pente du décor se marche"
    );
    assert!(
        !walkable(facing(SLOPE * 1.01)),
        "une pente plus raide que le décor ne se marche pas"
    );
    assert!(
        !walkable(Vec3::new(0.0, 0.0, -1.0)),
        "une dalle vue par dessous ne se marche pas"
    );
}

/// Un corps posé reste posé, et sa cote ne dérive pas.
///
/// **C'est le prédicat que la pesanteur met en danger**, et il manquerait à
/// l'épreuve précédente : une chute qui s'arrête peut très bien repartir à l'image
/// suivante, et un corps qui s'enfonce d'un cheveu par image finit sous le décor en
/// une minute. Ce qui est exigé est donc l'immobilité verticale sur la durée, à la
/// marge du balayage près.
#[test]
fn un_corps_pose_ne_derive_pas() {
    /// Combien d'images le corps reste sans qu'on lui demande rien.
    const FRAMES: usize = 600;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let mut body = Body::stand(HALF, &grid, &map, grid.start());
        let start = body.centre.z;

        for _ in 0..FRAMES {
            body.advance(&map, Vec3::new(0.0, 0.0, 0.0), DT);
        }

        assert!(
            body.grounded(&map),
            "graine {seed:#x} : après {FRAMES} images sans rien demander, \
             le corps ne repose plus sur rien en {:?}",
            body.centre
        );
        assert!(
            (body.centre.z - start).abs() <= sweep_skin(HALF),
            "graine {seed:#x} : la cote a dérivé de {} en {FRAMES} images",
            body.centre.z - start
        );
    }
}

/// Une longue station au sol ne charge pas la chute qui suivra.
///
/// **C'est l'épreuve qui paie la remise à zéro**, et elle a été écrite parce que
/// rien ne la payait : la pesanteur s'intègre à chaque image, donc un corps posé qui
/// garderait sa vitesse l'accumulerait pendant toute la marche. Au bout de dix
/// secondes elle vaut deux cents unités par seconde, et le premier rebord quitté
/// donnerait une plongée de trois mètres en une image — un corps téléporté vers le
/// bas, là où le sol l'avait simplement retenu.
///
/// **La mesure est un rapport et non une valeur** : la descente de la première image
/// après la station se compare à celle d'un corps frais lâché de la même hauteur.
/// Elle ne dépend donc ni de la pesanteur retenue ni de la cadence.
#[test]
fn une_station_au_sol_ne_charge_pas_la_chute() {
    /// Combien d'images le corps passe au sol avant d'être relevé.
    const FRAMES: usize = 600;
    /// De combien on le relève ensuite.
    const LIFT: f32 = 1.0;

    let still = Vec3::new(0.0, 0.0, 0.0);

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let at = grid.start();

        // La référence : un corps qui n'a rien vécu, lâché d'un mètre.
        let mut fresh = aloft(&grid, at, LIFT);
        let was = fresh.centre.z;
        fresh.advance(&map, still, DT);
        let reference = was - fresh.centre.z;

        // Le même, mais après dix secondes passées posé, puis relevé d'autant.
        let mut rested = Body::stand(HALF, &grid, &map, at);
        for _ in 0..FRAMES {
            rested.advance(&map, still, DT);
        }
        rested.centre.z += LIFT;
        let was = rested.centre.z;
        rested.advance(&map, still, DT);
        let dropped = was - rested.centre.z;

        assert!(
            dropped <= reference * 2.0,
            "graine {seed:#x} : après {FRAMES} images au sol, la première image de \
             chute descend de {dropped} contre {reference} pour un corps frais"
        );
    }
}

/// Une cage se gravit de bout en bout, qu'elle soit en marches ou en rampe.
///
/// **C'est le prédicat du lot, et il ne distingue pas les deux formes** : une volée
/// se monte par le seuil de franchissement, une rampe par le critère de surface
/// marchable, et l'épreuve n'a pas à savoir laquelle elle a tirée — ce qui est exigé
/// est d'arriver à l'étage du dessus. Les deux formes sont présentes dans le même
/// labyrinthe, donc les deux chemins sont exercés.
///
/// **On pousse vers le côté de la montée, pas vers une cote.** Marcher contre la
/// pente est exactement ce que fait un joueur ; viser une altitude serait écrire le
/// résultat dans l'épreuve.
///
/// **Le départ est la case d'accès et non celle de la cage**, et c'est une mesure :
/// `ground` rend la cote du sol **au centre** d'une case, donc une rampe y est déjà
/// à mi-hauteur et une volée au milieu de ses marches. Partir de là tronquerait la
/// montée de la moitié — relevé à 1,41 sur une rampe avant correction.
#[test]
fn une_cage_se_gravit_de_bout_en_bout() {
    /// Ce qu'une image parcourt, en unités de monde.
    const STEP: f32 = 0.05;
    /// Combien d'images la montée a pour se faire.
    ///
    /// Trois cents, soit cinq secondes : une volée fait quatorze marches et la
    /// largeur d'une case, donc le compte est large — ce qui est cherché est un
    /// blocage, pas une cadence.
    const FRAMES: usize = 300;

    for seed in SEEDS {
        let (grid, map) = maze(seed);

        let mut seen = 0;

        for stair in grid.stairs() {
            // La case d'où l'on entre dans la cage : en amont de la montée, plate, et
            // ouverte de ce côté.
            let Some(access) = grid.neighbour(stair.foot, stair.climb.facing()) else {
                continue;
            };
            if !plain(&grid, access) || grid.has_wall(access, stair.climb) {
                continue;
            }
            seen += 1;

            // En ligne, puis **en biais vers chacune des deux tangentes** : c'est le
            // régime réel, personne ne garde un cap parfait dans une cage, et c'est
            // le cas de biais qui a trouvé une rampe bloquée alors que la montée en
            // ligne passait.
            for aim in [None, Some(0), Some(1)] {
                let mut body = Body::stand(HALF, &grid, &map, access);
                let from = body.centre;
                let line = unit(stair.climb) * STEP;
                let push = match aim.map(|which| across(stair.climb)[which]) {
                    // Normalisé, pour qu'un pas de biais parcoure la même distance
                    // qu'un pas en ligne : sinon il avance de moitié en plus et
                    // l'épreuve mesurerait la cadence au lieu du chemin.
                    Some(tangent) => {
                        (unit(stair.climb) + unit(tangent))
                            * (STEP * std::f32::consts::FRAC_1_SQRT_2)
                    }
                    None => line,
                };

                // On entre toujours droit : viser de biais dès le palier éloigne de
                // l'entrée au lieu de l'emprunter.
                for _ in 0..FRAMES / 5 {
                    body.advance(&map, line, DT);
                }
                for _ in 0..FRAMES {
                    body.advance(&map, push, DT);
                }

                // Une marche de tolérance sur la hauteur d'étage : ce qui est cherché
                // est d'avoir gravi la volée, pas de s'être arrêté pile au palier.
                let climbed = body.centre.z - from.z;
                assert!(
                    climbed >= export::LEVEL - export::RISE,
                    "graine {seed:#x} : la cage {:?} de {:?} prise {} n'a monté que \
                     de {climbed}, partie de {from:?} et arrivée en {:?}",
                    stair.shape,
                    stair.foot,
                    match aim {
                        Some(which) => format!("de biais vers {:?}", across(stair.climb)[which]),
                        None => "en ligne".to_owned(),
                    },
                    body.centre
                );
            }
        }

        assert!(
            seen > 0,
            "graine {seed:#x} : aucune cage accessible éprouvée"
        );
    }
}

/// Un mur reste infranchissable, et le relèvement n'y change rien.
///
/// **C'est la non-régression du franchissement**, et elle est indispensable : la
/// politique monte d'une marche puis rejoue le pas, donc un relèvement adopté sans
/// mesure du gain ferait escalader les murs d'un quart de mètre à chaque image. Ce
/// qui est exigé est l'arrêt — la cote ne monte pas et le corps ne passe pas.
#[test]
fn un_mur_ne_s_escalade_pas() {
    /// Ce qu'une image parcourt, en unités de monde.
    const STEP: f32 = 0.05;
    /// Combien d'images on pousse contre le mur.
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

            let mut body = Body::stand(HALF, &grid, &map, at);
            let from = body.centre;
            let push = unit(side) * STEP;
            for _ in 0..FRAMES {
                body.advance(&map, push, DT);
            }
            seen += 1;

            assert!(
                body.centre.z - from.z <= sweep_skin(HALF),
                "contre le mur {side:?} de {at:?}, le corps a monté de {}",
                body.centre.z - from.z
            );
            assert!(
                (body.centre - from).dot(unit(side)) <= export::INNER / 2.0,
                "contre le mur {side:?} de {at:?}, le corps a avancé de {} \
                 et se trouve en {:?}",
                (body.centre - from).dot(unit(side)),
                body.centre
            );
        }
    }

    assert!(seen > 0, "aucun mur éprouvé");
}

/// Une cage se descend sans jamais quitter le sol.
///
/// **C'est l'épreuve du collage au sol, et elle mesure le régime et non l'arrivée** :
/// descendre finit toujours par arriver en bas, en marchant comme en tombant. Ce qui
/// sépare les deux est le **contact à chaque image** — un corps qui descend en chute
/// libre est en l'air la plupart du temps, et c'est ce qui se voyait à l'écran.
///
/// Le chiffre qui l'explique : un pas de cinq centièmes fait perdre au sol cinquante-
/// huit millièmes sur une rampe, et un quart de mètre au bord d'un giron. Les deux
/// dépassent la tolérance de contact, donc sans collage la pesanteur reprend à chaque
/// pas.
#[test]
fn une_cage_se_descend_sans_quitter_le_sol() {
    /// Ce qu'une image parcourt, en unités de monde.
    const STEP: f32 = 0.05;
    /// Combien d'images la descente a pour se faire.
    const FRAMES: usize = 300;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let mut seen = 0;

        for stair in grid.stairs() {
            // Le palier d'en haut : la case où la montée débouche, au-delà de la tête
            // de la cage.
            let Some(top) = grid.neighbour(stair.head(), stair.climb) else {
                continue;
            };
            if !plain(&grid, top) || grid.has_wall(top, stair.climb.facing()) {
                continue;
            }
            seen += 1;

            let mut body = Body::stand(HALF, &grid, &map, top);
            let from = body.centre;
            let push = unit(stair.climb.facing()) * STEP;

            for frame in 0..FRAMES {
                body.advance(&map, push, DT);
                assert!(
                    body.grounded(&map),
                    "graine {seed:#x} : en descendant la cage {:?} de {:?}, le corps \
                     a quitté le sol à l'image {frame}, en {:?}",
                    stair.shape,
                    stair.foot,
                    body.centre
                );
            }

            let descended = from.z - body.centre.z;
            assert!(
                descended >= export::LEVEL - export::RISE,
                "graine {seed:#x} : la cage {:?} de {:?} n'a descendu que de \
                 {descended} en {FRAMES} images",
                stair.shape,
                stair.foot
            );
        }

        assert!(
            seen > 0,
            "graine {seed:#x} : aucune cage descendable éprouvée"
        );
    }
}

/// Un corps posé sur une pente n'y glisse pas tout seul.
///
/// **C'est l'épreuve qui paie le critère de surface marchable**, et elle a été
/// écrite parce que rien ne le payait : durci jusqu'à refuser une rampe, le critère
/// laissait toutes les autres épreuves vertes, **y compris la montée** — le
/// franchissement gravit une pente par relèvements successifs, comme une suite de
/// marches, donc il masque entièrement le critère.
///
/// Ce que le critère décide n'est donc pas la montée mais le **repos** : refusée, la
/// pente n'est plus un sol, la pesanteur s'applique en permanence et la glissade
/// convertit chaque image en descente le long du plan. On monterait pour redescendre
/// dès qu'on s'arrête.
#[test]
fn un_corps_sur_une_pente_ne_glisse_pas() {
    /// Combien d'images le corps reste sans qu'on lui demande rien.
    const FRAMES: usize = 300;

    let still = Vec3::new(0.0, 0.0, 0.0);

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let mut seen = 0;

        for stair in grid.stairs() {
            if stair.shape != Shape::Ramp {
                continue;
            }
            seen += 1;

            let mut body = Body::stand(HALF, &grid, &map, stair.foot);
            let start = body.centre.z;
            for _ in 0..FRAMES {
                body.advance(&map, still, DT);
            }

            assert!(
                body.centre.z >= start - sweep_skin(HALF),
                "graine {seed:#x} : posé sur la rampe de {:?} à {start}, le corps a \
                 glissé jusqu'à {} en {FRAMES} images",
                stair.foot,
                body.centre.z
            );
        }

        assert!(seen > 0, "graine {seed:#x} : aucune rampe éprouvée");
    }
}

/// Le relèvement refuse un mur plein, et c'est lui qui le dit.
///
/// **Elle existe parce que rien d'autre ne paie la mesure du gain.** Adopter le
/// chemin relevé sans le comparer laisse toutes les autres épreuves vertes : contre
/// un mur, la descente finale ramène le corps au sol, donc le relèvement **égale**
/// l'arrêt au lieu de l'empirer. Ce que la comparaison refuse ne se voit donc qu'ici,
/// à l'appel.
///
/// **Et ce qu'elle garde n'est pas une précaution de style** : rien ne prouve qu'un
/// chemin relevé avance autant qu'un chemin au sol — une saillie à hauteur de marche
/// suffirait à le raccourcir —, là où le garde contre un recul de glissade avait pu
/// être retiré sur preuve. Ici il n'y a pas de preuve, seulement un décor qui ne
/// produit pas le cas.
#[test]
fn le_relevement_refuse_un_mur_plein() {
    /// Ce qu'une image parcourt, en unités de monde.
    const STEP: f32 = 0.05;

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

            // Collé au mur : c'est l'état d'où le franchissement est tenté.
            let mut body = Body::stand(HALF, &grid, &map, at);
            let push = unit(side) * STEP;
            for _ in 0..40 {
                body.advance(&map, push, DT);
            }

            let stopped = body.slide(&map, body.centre, body.centre + push);
            assert!(
                stopped.stopped,
                "contre le mur {side:?} de {at:?}, le pas n'est pas arrêté par du \
                 non-marchable"
            );
            assert!(
                body.climb(&map, push, stopped.at).is_none(),
                "contre le mur {side:?} de {at:?}, le relèvement a été adopté"
            );
            seen += 1;
        }
    }

    assert!(seen > 0, "aucun mur éprouvé");
}

/// Le gabarit de la créature garde la même marge de glissade que le joueur.
///
/// **Les deux bornes du module sont relevées pour une seule boîte**, et c'est ce
/// qui les rendrait supposées pour une autre : [`SLIDES`] a été mesuré contre le
/// gabarit du joueur, et rien ne dit qu'une boîte plus étroite présente le même
/// nombre de plans dans un coin. Elle en présente moins ou autant — une boîte fine
/// touche moins de choses à la fois —, mais c'est à mesurer, pas à déduire.
#[test]
fn le_gabarit_de_la_creature_garde_la_marge_de_glissade() {
    let (grid, map) = maze(SEEDS[0]);
    let mut worst = 0;
    let mut whence = None;

    for at in cases(&grid) {
        if !plain(&grid, at) {
            continue;
        }
        for bearing in bearings() {
            let body = Body::stand(monster::HALF, &grid, &map, at);
            let wanted = body.centre + bearing * export::CELL;
            let planes = body.slide(&map, body.centre, wanted).planes;

            if planes > worst {
                worst = planes;
                whence = Some((at, bearing));
            }
        }
    }

    assert!(
        worst < SLIDES,
        "une glissade de la créature a consommé les {SLIDES} plans de la borne, \
         en {whence:?}"
    );
}

/// Le décor joué tient là où la boîte de la créature garde son jeu de collision.
///
/// **La même épreuve que pour le joueur, et pour la même raison** : le jeu que le
/// balayage laisse à une boîte se perd avec l'éloignement de l'origine, et il dépend
/// de **sa taille**. Une boîte plus petite le perd plus tôt, donc la borne du joueur
/// ne vaut pas pour elle — c'est précisément le cas que cette épreuve couvre.
#[test]
fn le_decor_joue_garde_le_jeu_de_la_creature() {
    let (width, height, levels) = crate::MAZE.extent;

    let far = (width as f32 * export::CELL)
        .max(height as f32 * export::CELL)
        .max((levels - 1) as f32 * export::LEVEL + export::CEILING);

    assert!(
        far < sweep_reach(monster::HALF),
        "le décor va jusqu'à {far}, et la boîte de la créature perd son jeu à {}",
        sweep_reach(monster::HALF)
    );
}

/// Un autre gabarit que celui du joueur emprunte le même chemin.
///
/// **C'est l'épreuve qui paie l'extraction**, et sans elle le lot n'aurait déplacé
/// du code sans rien montrer : tout le reste d'ici éprouve le gabarit du joueur,
/// donc une demi-étendue restée en constante de module les laisserait toutes vertes.
///
/// **Le gabarit est choisi pour être faux à l'autre** : plus large et plus bas, de
/// quoi que chacune des trois assertions morde si la boîte balayée n'était pas la
/// sienne. Posé par le calcul d'un corps deux fois plus haut, il flotterait d'un
/// demi-mètre au-dessus du sol, et arrêté sur la boîte du joueur il s'approcherait
/// du mur d'un quart de mètre de plus que son volume ne le permet — donc dans le
/// solide.
///
/// **Les trois assertions sont les trois usages que `E3.5` fera du module** : se
/// poser sur une case, être arrêté par le décor, et glisser le long de ce qui
/// l'arrête. Une créature n'en demande pas d'autre.
#[test]
fn un_autre_gabarit_emprunte_le_meme_chemin() {
    /// Le gabarit éprouvé, en unités de monde : plus large et plus bas que le joueur.
    ///
    /// Il reste sous les deux bornes du décor — un mètre dix de côté dans une
    /// cellule de trois mètres, quatre-vingts centimètres de haut sous un plafond de
    /// trois et quart.
    const OTHER: Vec3 = Vec3::new(0.55, 0.55, 0.4);
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
                let mut body = Body::stand(OTHER, &grid, &map, at);

                assert!(
                    body.grounded(&map),
                    "posé en {at:?}, le corps de {OTHER:?} ne repose sur rien \
                     en {:?}",
                    body.centre
                );

                let before = body.centre;
                let aim = unit(side) * export::CELL + unit(tangent) * ALONG;
                body.advance(&map, aim, DT);

                assert!(
                    !solid(&map, &body),
                    "poussé contre le mur {side:?} de {at:?}, le corps de {OTHER:?} \
                     est entré dans le décor en {:?}",
                    body.centre
                );
                let kept = (body.centre - before).dot(unit(tangent));
                assert!(
                    kept >= ALONG - sweep_skin(OTHER),
                    "contre le mur {side:?} de {at:?}, le pas du corps de {OTHER:?} \
                     vers {tangent:?} n'a gardé que {kept} de {ALONG}"
                );
                seen += 1;
            }
        }
    }
    assert!(seen > 0, "aucun mur éprouvé : la graine n'en porte pas");
}
