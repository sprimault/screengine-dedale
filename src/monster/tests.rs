// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de la pose d'une créature et de l'ancrage de son dessin.
//!
//! **Ce que le lot a de visible ne s'éprouve pas ici** : la demi-étendue du
//! sprite, la distance à laquelle une silhouette se lit et la lisibilité des huit
//! vues se jugent en lançant le jeu. Ce qui se teste malgré la fenêtre est la pose
//! — dans le solide ou non, posée ou flottante — et la relation entre le volume et
//! le dessin.
//!
//! **Les cotes de la table se remesurent sur les planches, elles ne se relisent
//! pas.** Marge de cadrage et foulée sont des relevés, donc une planche refaite par
//! la chaîne doit faire rougir la table plutôt que de passer en silence : c'est tout
//! ce que `la_marge_annoncee_est_la_plus_frequente` et
//! `la_foulee_annoncee_est_celle_de_la_planche` existent pour tenir.
//!
//! **Aucune empreinte d'image.** La conformance est l'affaire du moteur, qui la
//! tient sur ses propres scènes ; une seconde suite d'empreintes ici le mesurerait
//! deux fois et casserait à chaque changement d'habillage.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use std::sync::OnceLock;

use super::*;
use crate::maze::export;
use crate::maze::grid::Settings;
use crate::scene::{Scenery, View};
use crate::test_support::{HEIGHT, SEEDS, WIDTH, context, frame, maze, snapshot};
use screengine_play::FreeCamera;
use screengine_play::screengine::BYTES_PER_PIXEL;

/// Les deux vues de profil d'une planche : à un quart et à trois quarts de tour.
///
/// **C'est là et nulle part ailleurs que la foulée se lit** : de face, les deux
/// appuis se superposent et l'écartement ne se voit pas.
const PROFILES: [u32; 2] = [2, 6];

/// La largeur encrée d'une vignette, en texels — toute la silhouette, pas ses pieds.
///
/// **Elle ne se déduit pas de [`footprint`]**, qui ne regarde que les quatre lignes du
/// bas : ce qu'on vise est le corps, et un bras tendu ou une aile élargissent la cible
/// sans qu'un pied ait bougé. Rend zéro pour une vignette vide, qu'aucune planche du
/// dépôt ne porte.
fn drawn_width(sheet: &Texture, view: u32, frame: u32) -> u32 {
    let side = FRAME as u32;
    let (bu, bv) = (frame * side, view * side);
    let inked =
        |du: u32| (0..side).any(|dv| sheet.texel(0, (bu + du) as i32, (bv + dv) as i32) >> 24 != 0);

    let span = (0..side).filter(|&du| inked(du));
    match (span.clone().min(), span.max()) {
        (Some(left), Some(right)) => right - left + 1,
        _ => 0,
    }
}

/// La largeur du **corps** d'une vignette, en texels : la médiane des largeurs de ligne.
///
/// **La médiane et non le maximum**, parce que ce qu'on cherche est ce qu'un joueur
/// prend pour le corps : un bras tendu ou une aile élargit l'enveloppe sur quelques
/// lignes, et un volume qui la couvrirait rendrait touchable le vide entre les membres.
fn trunk_width(sheet: &Texture, view: u32, frame: u32) -> u32 {
    let side = FRAME as u32;
    let (bu, bv) = (frame * side, view * side);
    let solid = |du: u32, dv: u32| sheet.texel(0, (bu + du) as i32, (bv + dv) as i32) >> 24 != 0;

    let mut widths: Vec<u32> = (0..side)
        .filter_map(|dv| {
            let span = (0..side).filter(|&du| solid(du, dv));
            match (span.clone().min(), span.max()) {
                (Some(left), Some(right)) => Some(right - left + 1),
                _ => None,
            }
        })
        .collect();

    widths.sort_unstable();
    widths.get(widths.len() / 2).copied().unwrap_or_default()
}

/// Combien de lignes vides la planche laisse **au-dessus** de la silhouette.
///
/// Le pendant de [`hollow`] par le haut, et les deux ensemble donnent la hauteur
/// encrée : c'est elle qu'un volume touchable doit couvrir, pas le côté de la vignette.
fn crown(sheet: &Texture, view: u32, frame: u32) -> u32 {
    let side = FRAME as u32;
    let (u, v) = (frame * side, view * side);

    (0..side)
        .find(|&row| {
            (0..side).any(|col| sheet.texel(0, (u + col) as i32, (v + row) as i32) >> 24 != 0)
        })
        .unwrap_or(side)
}

/// Combien de lignes vides la planche laisse sous la silhouette d'une vignette.
///
/// **L'alpha décide**, et il est binaire : la planche se charge masquée, donc un
/// texel est opaque ou ne l'est pas, sans seuil à choisir ici.
fn hollow(sheet: &Texture, view: u32, frame: u32) -> u32 {
    let side = FRAME as u32;
    let (u, v) = (frame * side, view * side);

    for row in (0..side).rev() {
        for col in 0..side {
            if sheet.texel(0, (u + col) as i32, (v + row) as i32) >> 24 != 0 {
                return side - 1 - row;
            }
        }
    }

    side
}

/// La largeur de l'empreinte des pieds d'une vignette, en texels.
///
/// **Elle se relève au ras du sol**, sur les quatre dernières lignes opaques : un
/// genou plié ou un bras baissé élargirait la mesure sans qu'un pied ait bougé.
///
/// **Deux épreuves la demandent** — la tache d'ombre pour sa largeur, la foulée pour
/// son écartement —, et c'est le même relevé : séparées, les deux finiraient par ne
/// plus mesurer la même chose. Rend zéro pour une vignette vide, qu'aucune planche du
/// dépôt ne porte.
fn footprint(sheet: &Texture, view: u32, frame: u32) -> u32 {
    let side = FRAME as u32;
    let (bu, bv) = (frame * side, view * side);
    let solid = |du: u32, dv: u32| sheet.texel(0, (bu + du) as i32, (bv + dv) as i32) >> 24 != 0;

    let Some(floor) = (0..side)
        .rev()
        .find(|&dv| (0..side).any(|du| solid(du, dv)))
    else {
        return 0;
    };
    let feet = floor.saturating_sub(3)..=floor;
    let span = (0..side).filter(|&du| feet.clone().any(|dv| solid(du, dv)));

    match (span.clone().min(), span.max()) {
        (Some(left), Some(right)) => right - left + 1,
        _ => 0,
    }
}

/// Les trois créatures naissent hors du solide, quelle que soit la graine.
///
/// **Elles empruntent le chemin du joueur et doivent donc en tirer la même
/// garantie** : la pose se résout par un balayage, et un gabarit plus étroit ne
/// change pas ce que le décor laisse de place.
///
/// **Les trois et non la première**, et c'est ce que la population a ajouté : leurs
/// cases viennent d'un parcours des passages, donc la deuxième et la troisième
/// tombent où le labyrinthe les met — y compris au pied d'une cage, où une boîte
/// posée à la seule cote du sol pénètre les marches.
#[test]
fn le_demon_nait_hors_du_solide() {
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let monsters = population(&grid, &map).expect("planches du dépôt valides");

        for monster in &monsters {
            let centre = monster.at();
            let cell = monster.body.cell();
            assert_ne!(
                cell,
                0,
                "graine {seed:#x} : {} naît hors du décor",
                monster.name()
            );

            // Un pas nul ne rencontre rien, par contrat : la plus courte sonde qui
            // dise quelque chose est une descente.
            let below = Vec3::new(centre.x, centre.y, centre.z - 1.0);
            let hit = map
                .sweep(cell, HALF, centre, below)
                .expect("la cellule du démon existe");
            assert!(
                !hit.start_solid,
                "graine {seed:#x} : {} naît dans le solide en {centre:?}, \
                 contre la surface {} de normale {:?}",
                monster.name(),
                hit.surface,
                hit.normal
            );
        }
    }
}

/// Et elles naissent **posées** : ce qu'elles ont sous les pieds les arrête aussitôt.
///
/// La distinction avec l'épreuve précédente est celle qui sépare une créature
/// debout d'une créature qui flotte. Une pose relevée d'un centimètre les
/// passerait toutes les deux, et le démon planerait au-dessus des dalles — ce qui
/// se verrait, mais seulement de près et de côté.
#[test]
fn le_demon_nait_pose_sur_son_sol() {
    /// La course de la sonde, en unités de monde.
    const PROBE: f32 = 1.0;

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let monsters = population(&grid, &map).expect("planches du dépôt valides");

        for monster in &monsters {
            let centre = monster.at();
            let below = Vec3::new(centre.x, centre.y, centre.z - PROBE);
            let hit = map
                .sweep(monster.body.cell(), HALF, centre, below)
                .expect("la cellule du démon existe");
            assert!(
                hit.fraction * PROBE <= HALF.z / 1024.0,
                "graine {seed:#x} : {} descend de {} avant de toucher, \
                 depuis {centre:?}",
                monster.name(),
                hit.fraction * PROBE
            );
            assert!(
                hit.normal.z > 0.5,
                "graine {seed:#x} : ce qui porte {} a pour normale {:?}",
                monster.name(),
                hit.normal
            );
        }
    }
}

/// Deux créatures naissent sur deux cases, jamais sur la même.
///
/// **C'est le marquage des cases vues qui le donne**, et rien d'autre : sans lui, un
/// parcours qui revient sur ses pas rend deux fois la même case, et deux silhouettes
/// se superposent.
///
/// **Ce que le marquage débranché produit n'est pas un doublon, c'est pire, et c'est
/// mesuré** : le parcours revient alors sur la case de départ et y pose une
/// silhouette — donc dans le joueur, qui y naît aussi. Le doublon, lui, demande un
/// départ à deux passages dont les deux voisines sont reliées par une boucle, et
/// aucune des soixante-quatre grilles éprouvées ne le présente.
///
/// **D'où les deux temps** : les six graines du dépôt vérifient les volumes posés —
/// ce qu'une épreuve sur les cases seules ne dirait pas, deux cases distinctes
/// pouvant porter deux corps qui se touchent —, puis soixante-quatre grilles
/// vérifient les cases rendues.
#[test]
fn chaque_demon_nait_sur_sa_case() {
    /// Combien de grilles on engendre pour le second temps.
    const GRIDS: u64 = 64;

    for seed in 0..GRIDS {
        let grid = Grid::generate(Settings {
            extent: (16, 16, 2),
            seed,
            stairs: 6,
            ramps: 2,
            loops: 8,
        });
        let spots = spots(&grid);
        assert_eq!(spots.len(), FIGURES.len(), "graine {seed}");

        for (rank, spot) in spots.iter().enumerate() {
            assert!(
                !spots[..rank].contains(spot),
                "graine {seed} : le parcours rend deux fois la case {spot:?}, \
                 donc deux silhouettes s'y superposent"
            );
            assert_ne!(
                *spot,
                grid.start(),
                "graine {seed} : une silhouette naît sur la case de départ, \
                 donc dans le joueur"
            );
        }
    }

    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let monsters = population(&grid, &map).expect("planches du dépôt valides");
        assert_eq!(monsters.len(), FIGURES.len());

        for (rank, monster) in monsters.iter().enumerate() {
            for other in monsters.iter().skip(rank + 1) {
                assert!(
                    !meets(monster.at(), other.at()),
                    "graine {seed:#x} : {} et {} naissent au même endroit, \
                     en {:?} et {:?}",
                    monster.name(),
                    other.name(),
                    monster.at(),
                    other.at()
                );
            }
        }
    }
}

/// Le bas du dessin tombe sur les pieds du corps, au cadrage près.
///
/// **C'est la relation que rien d'autre ne tient** : le volume et le dessin sont
/// deux choses, et seule cette fonction les raccorde. Un signe inversé ou un
/// facteur deux oublié ferait flotter la créature ou l'enterrerait jusqu'aux
/// genoux — visible à l'écran, mais seulement une fois qu'on sait quoi regarder.
///
/// **Les six cadrages et non un seul** : la marge appartient à la silhouette **et**
/// au cycle, donc l'ancrage se vérifie sur chaque couple. Une table lue de travers —
/// l'indice d'une silhouette pris pour celui d'une autre — passerait sur un seul cas.
///
/// **À un millième de texel près, et pas à l'égalité exacte** : les deux membres
/// sont la même expression réordonnée, mais le retrait ne tombe pas juste en
/// binaire et les sépare d'un arrondi. L'écart toléré se dit en texels parce que
/// c'est la seule unité qui ait un sens ici — un texel vaut trois centimètres à
/// l'échelle du décor, donc mille fois ce qu'on laisse passer.
#[test]
fn le_sprite_pose_ses_pieds_au_sol() {
    /// L'écart toléré, en unités de monde : un millième de texel.
    const SLACK: f32 = 2.0 * SPRITE_HALF / FRAME / 1000.0;

    for figure in &FIGURES {
        for motion in [Motion::Idle, Motion::Walk] {
            for centre in [
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(12.0, -3.5, 7.25),
                Vec3::new(-40.0, 60.0, -2.0),
            ] {
                let placed = anchor(centre, figure, motion);

                assert_eq!(placed.x, centre.x);
                assert_eq!(placed.y, centre.y);

                // Le bas de la vignette, puis ce que le cadrage y laisse de vide :
                // la somme des deux est la cote des pieds dessinés.
                let bottom = placed.z - SPRITE_HALF;
                let drawn = bottom + figure.margin(motion) / FRAME * (2.0 * SPRITE_HALF);

                assert!(
                    (drawn - (centre.z - HALF.z)).abs() <= SLACK,
                    "{} en {motion:?} : pour un corps centré en {centre:?}, les pieds \
                     dessinés sont en {drawn} et le bas du corps en {}",
                    figure.name,
                    centre.z - HALF.z
                );
            }
        }
    }
}

/// Vrai si la créature est dans le solide, par une sonde que le moteur tranche.
///
/// **Un pas, et non une position** : le balayage ne se prononce que sur un
/// mouvement, donc la sonde descend. Un départ solide se signale avant qu'elle
/// serve.
/// Une créature seule, de la première silhouette, posée sur cette case.
///
/// **La population ne sert pas ici** : elle place trois créatures près de l'entrée et
/// leur donne des caps répartis, là où le recul se mesure sur une seule, à un endroit
/// choisi et sans voisine pour la gêner.
///
/// Le cap est celui que le rang zéro donne, soit le `+X` ; les épreuves qui en veulent
/// un autre l'écrivent.
fn lone(grid: &Grid, map: &World, at: (u32, u32, u32)) -> Monster {
    Monster {
        figure: &FIGURES[0],
        idle: stub(),
        walk: stub(),
        dead: stub(),
        shadow: stub(),
        body: Body::stand(HALF, grid, map, at),
        facing: 0.0,
        motion: Motion::Idle,
        phase: 0.0,
        hindered: 0,
        recoil: None,
        life: LIFE,
    }
}

/// Une planche d'un texel, partagée par toutes les créatures d'épreuve.
///
/// **Le déplacement ne lit aucune planche** : la marche et le recul ne touchent qu'au
/// corps, au cap et au cycle, et les cotes qui décident — la foulée, le gabarit —
/// viennent de la table des silhouettes. Ce qui regarde les planches, ce sont la
/// sélection de vue et le rendu, qui ont leurs propres épreuves et les vraies images.
///
/// **Mesuré : décoder deux images de cinq cent douze texels par créature coûtait
/// cinquante-huit secondes** sur le corpus des cloisons, pour des octets que rien
/// n'échantillonne. Une épreuve qu'on hésite à lancer est une épreuve qu'on ne lance
/// pas.
fn stub() -> Arc<Texture> {
    static SHEET: OnceLock<Arc<Texture>> = OnceLock::new();
    SHEET
        .get_or_init(|| {
            Arc::new(
                Texture::load(1, 1, &[0xFF, 0xFF, 0xFF, 0xFF])
                    .expect("un texel blanc est une texture tenable"),
            )
        })
        .clone()
}

/// La première case plate de ce labyrinthe.
///
/// **Plate, donc sans cage** : le sol d'une cage monte, et une créature posée sur un
/// palier y part d'un départ dans le solide que le balayage refuse de départager.
fn plain_case(grid: &Grid) -> (u32, u32, u32) {
    crate::test_support::cases(grid)
        .into_iter()
        .find(|at| crate::test_support::plain(grid, *at))
        .expect("le labyrinthe a une case plate")
}

fn buried(map: &World, monster: &Monster) -> bool {
    let centre = monster.at();
    let below = Vec3::new(centre.x, centre.y, centre.z - 1.0);

    map.sweep(monster.body.cell(), HALF, centre, below)
        .is_some_and(|hit| hit.start_solid)
}

/// Une créature qui marche ne traverse jamais le décor.
///
/// **C'est le prédicat du lot**, et il part du régime réel : des pas d'image
/// enchaînés sur une demi-minute, pas un saut d'une case. Les créatures traversent
/// couloirs, cages et paliers sans qu'aucune ligne d'ici ne décide de leur route, et
/// ce qui est exigé est qu'elles restent dans une cellule connue et hors du solide à
/// **chaque** image — pas seulement à la fin, où un aller-retour effacerait un
/// passage à travers un mur.
#[test]
fn un_demon_qui_marche_ne_traverse_pas_le_decor() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;
    /// Combien d'images les créatures marchent.
    const FRAMES: usize = 900;

    for seed in [SEEDS[0], SEEDS[4]] {
        let (grid, map) = maze(seed);
        let mut monsters = population(&grid, &map).expect("planches du dépôt valides");

        for frame in 0..FRAMES {
            stroll(&mut monsters, &map, DT);

            for monster in &monsters {
                assert_ne!(
                    monster.body.cell(),
                    0,
                    "graine {seed:#x} : à l'image {frame}, {} a quitté le décor \
                     en {:?}",
                    monster.name(),
                    monster.at()
                );
                assert!(
                    !buried(&map, monster),
                    "graine {seed:#x} : à l'image {frame}, {} est dans le solide \
                     en {:?}",
                    monster.name(),
                    monster.at()
                );
            }
        }
    }
}

/// Un recul pousse la créature dans la direction du coup, sur la distance annoncée.
///
/// **La distance est ce qui se vérifie**, pas le fait de bouger : c'est elle qui décide
/// qu'un coup se lit, et un recul d'un dixième de ce qu'il annonce passerait un test de
/// présence sans rien montrer à l'écran.
///
/// **En terrain dégagé et sans marche**, pour que la mesure porte sur le recul seul : la
/// créature est posée au centre d'une case plate et le coup la pousse vers une case
/// voisine ouverte.
#[test]
fn un_recul_pousse_de_la_distance_annoncee() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let open = crate::test_support::cases(&grid)
        .into_iter()
        .find(|at| {
            crate::test_support::plain(&grid, *at)
                && crate::test_support::sides()
                    .iter()
                    .any(|side| !grid.has_wall(*at, *side))
        })
        .expect("le labyrinthe a une case plate ouverte");
    let side = crate::test_support::sides()
        .into_iter()
        .find(|side| !grid.has_wall(open, *side))
        .expect("cette case est ouverte quelque part");

    let mut monster = lone(&grid, &map, open);
    let before = monster.at();
    let push = crate::test_support::unit(side);
    monster.knock(push);

    // Le recul court sur sa durée, puis la marche reprend : on s'arrête à la dernière
    // image qui lui appartient encore.
    let frames = (RECOIL_TIME / DT).ceil() as usize;
    for _ in 0..frames {
        monster.walk(&map, DT, false);
    }

    let gone = monster.at() - before;
    let flat = Vec3::new(gone.x, gone.y, 0.0);
    let expected = RECOIL_SPEED * RECOIL_TIME;
    assert!(
        (flat.dot(flat).sqrt() - expected).abs() <= expected / 8.0,
        "le recul vers {side:?} a parcouru {} et non {expected}",
        flat.dot(flat).sqrt()
    );
    assert!(
        flat.dot(push) > 0.0,
        "le recul vers {side:?} est parti dans l'autre sens : {flat:?}"
    );
}

/// La distance d'un recul ne dépend pas de la cadence.
///
/// **Sans la troncature du dernier pas, elle en dépendrait** : un recul compté en images
/// parcourrait deux fois plus à cent vingt images par seconde qu'à soixante, et le coup
/// serait plus fort sur une machine rapide. C'est le genre d'écart qu'on ne voit jamais
/// sur son propre poste.
#[test]
fn la_distance_d_un_recul_ne_depend_pas_de_la_cadence() {
    let (grid, map) = maze(SEEDS[0]);
    let at = plain_case(&grid);

    let push = Vec3::new(1.0, 0.0, 0.0);
    let run = |dt: f32| {
        let mut monster = lone(&grid, &map, at);
        let before = monster.at();
        monster.knock(push);
        for _ in 0..(RECOIL_TIME / dt).ceil() as usize {
            monster.walk(&map, dt, false);
        }
        let gone = monster.at() - before;
        Vec3::new(gone.x, gone.y, 0.0).dot(push)
    };

    let (slow, quick) = (run(1.0 / 30.0), run(1.0 / 120.0));
    assert!(
        (slow - quick).abs() <= RECOIL_SPEED * RECOIL_TIME / 16.0,
        "le recul vaut {slow} à trente images par seconde et {quick} à cent vingt"
    );
}

/// Un recul ne touche ni la patience de la créature, ni son cycle de marche.
///
/// **Les deux couplages que le lot existe pour couper**, et aucun des deux ne se verrait
/// en regardant : une patience remise à zéro fait qu'une créature reculée contre un mur
/// cesse de s'en détourner — ce qui se lit comme un défaut de la marche, des secondes
/// plus tard et ailleurs —, et une phase qui avance fait défiler les jambes d'une
/// créature poussée en arrière, ce qu'on prend pour un cycle mal réglé.
///
/// **La patience est portée à une valeur franche avant le coup** : à zéro, un recul qui
/// la remettrait à zéro ne se distinguerait pas d'un recul qui n'y touche pas.
#[test]
fn un_recul_ne_touche_ni_la_patience_ni_le_cycle() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let at = plain_case(&grid);
    let mut monster = lone(&grid, &map, at);

    monster.hindered = PATIENCE / 2;
    monster.motion = Motion::Walk;
    monster.phase = 0.25;
    let (patience, motion, phase) = (monster.hindered, monster.motion, monster.phase);

    monster.knock(Vec3::new(1.0, 0.0, 0.0));
    monster.walk(&map, DT, false);

    assert_eq!(
        monster.hindered, patience,
        "le recul a touché la patience, donc la créature cessera de se détourner"
    );
    assert_eq!(monster.motion, motion, "le recul a changé le cycle");
    assert_eq!(
        monster.phase, phase,
        "le recul a fait défiler le cycle, donc les jambes marchent en arrière"
    );
}

/// Un coup vertical ne soulève pas la créature, et ne la pousse pas non plus.
///
/// **La direction est ramenée à l'horizontale**, donc un tir en plongée pousse par sa
/// composante au sol — et un tir strictement vertical n'en a aucune. Soulever la
/// créature ou l'enfoncer serait montrer autre chose que ce qu'on veut : qu'elle
/// encaisse.
#[test]
fn un_coup_vertical_ne_souleve_rien() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let at = plain_case(&grid);
    let mut monster = lone(&grid, &map, at);

    monster.knock(Vec3::new(0.0, 0.0, 1.0));
    assert!(
        monster.recoil.is_none(),
        "un coup strictement vertical a imprimé un recul"
    );

    // Et un coup plongeant en imprime un, mais à plat : sans ce second cas, un refus
    // de tout passerait.
    let before = monster.at();
    monster.knock(Vec3::new(1.0, 0.0, -1.0));
    for _ in 0..(RECOIL_TIME / DT).ceil() as usize {
        monster.walk(&map, DT, false);
    }
    let gone = monster.at() - before;
    assert!(
        gone.x > 0.0,
        "un coup plongeant n'a pas poussé à l'horizontale : {gone:?}"
    );
}

/// Un coup pendant un recul le relance au lieu de s'y ajouter.
///
/// **Sinon une rafale composerait les vitesses** et la créature partirait d'un bond au
/// second coup. C'est la même clause que la pose de tir de l'arme, qui se relance.
#[test]
fn un_coup_pendant_un_recul_le_relance() {
    let (grid, map) = maze(SEEDS[0]);
    let at = plain_case(&grid);
    let mut monster = lone(&grid, &map, at);

    let push = Vec3::new(1.0, 0.0, 0.0);
    monster.knock(push);
    monster.walk(&map, 1.0 / 60.0, false);
    monster.knock(push);

    let recoil = monster.recoil.expect("le coup a relancé le recul");
    assert_eq!(
        recoil.left, RECOIL_TIME,
        "le second coup n'a pas remis la durée à son plein"
    );
    let speed = recoil.speed.dot(recoil.speed).sqrt();
    assert!(
        (speed - RECOIL_SPEED).abs() <= RECOIL_SPEED / 1024.0,
        "deux coups ont composé leur vitesse : {speed} au lieu de {RECOIL_SPEED}"
    );
}

/// Une créature reculée ne traverse pas le décor.
///
/// **Elle part d'une pose au contact**, et c'est la clause du projet : le centre d'une
/// case est à plus d'un mètre de toute paroi, donc un recul éprouvé de là ne verrait
/// jamais une créature poussée **dans** un mur — qui est pourtant le cas normal, une
/// créature longeant les parois.
///
/// **Ce qu'elle garde est que le recul passe par le corps**, et rien de plus : la
/// propriété est tenue par le type tant qu'il est le seul chemin de déplacement — la
/// pose du corps est privée, donc rien dans ce module ne peut la déplacer autrement.
/// Vérifié en centuplant la vitesse du recul : le balayage l'arrête quand même. Elle
/// attraperait un recul qu'on ferait un jour passer à côté du corps « pour que ça
/// glisse mieux », et c'est sa seule raison d'être.
///
/// **Deux graines suffisent donc**, là où le corpus entier ne mesurerait que son
/// propre temps : ce qui est éprouvé ne dépend pas de la forme du labyrinthe.
#[test]
fn une_creature_reculee_ne_traverse_pas_le_decor() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let mut walled = 0;
    for seed in [SEEDS[0], SEEDS[4]] {
        let (grid, map) = maze(seed);
        for at in crate::test_support::cases(&grid) {
            if !crate::test_support::plain(&grid, at) {
                continue;
            }
            for side in crate::test_support::sides() {
                if !grid.has_wall(at, side) {
                    continue;
                }
                walled += 1;

                let mut monster = lone(&grid, &map, at);
                // Au contact : on marche d'abord vers la cloison, puis on y est poussé.
                monster.facing = {
                    let (dx, dy, _) = side.step();
                    (dy as f32).atan2(dx as f32)
                };
                for _ in 0..60 {
                    monster.walk(&map, DT, false);
                }

                monster.knock(crate::test_support::unit(side));
                for _ in 0..(RECOIL_TIME / DT).ceil() as usize {
                    monster.walk(&map, DT, false);
                    assert_ne!(
                        monster.body.cell(),
                        0,
                        "graine {seed:#x}, case {at:?} : reculé vers {side:?}, \
                         le démon a quitté le décor en {:?}",
                        monster.at()
                    );
                    assert!(
                        !buried(&map, &monster),
                        "graine {seed:#x}, case {at:?} : reculé vers {side:?}, \
                         le démon est dans le solide en {:?}",
                        monster.at()
                    );
                }
            }
        }
    }

    assert!(
        walled > 500,
        "seules {walled} cloisons ont été éprouvées, le corpus n'en est pas un"
    );
}

/// Trois coups abattent une créature, et le troisième ne la recule pas.
///
/// **Les deux ensemble, parce que le second découle du premier** : le coup fatal fait
/// tomber là où on est, et un recul par-dessus ferait glisser le corps en tombant.
///
/// **Le compte se vérifie par en dessous aussi** : une créature encore debout au
/// troisième coup dirait que la vie ne décroît pas, et une créature tombée au deuxième
/// que le chiffre n'est pas celui qu'on croit.
#[test]
fn trois_coups_abattent_une_creature() {
    let (grid, map) = maze(SEEDS[0]);
    let mut monster = lone(&grid, &map, plain_case(&grid));
    let push = Vec3::new(1.0, 0.0, 0.0);

    for coup in 1..LIFE {
        monster.knock(push);
        assert!(
            !monster.fallen(),
            "la créature tombe au coup {coup} alors qu'elle en encaisse {LIFE}"
        );
        assert!(
            monster.recoil.is_some(),
            "le coup {coup} n'a pas reculé la créature"
        );
    }

    monster.knock(push);
    assert!(
        monster.fallen(),
        "la créature tient encore après {LIFE} coups"
    );
    assert_eq!(monster.motion, Motion::Dead, "elle ne joue pas sa mort");
    assert_eq!(monster.phase, 0.0, "son cycle de mort ne part pas du début");
    assert!(
        monster.recoil.is_none(),
        "le coup fatal recule la créature, donc elle glisse en tombant"
    );
}

/// Un seul coup est payé, c'est celui qui abat, et il rend la prime de la silhouette.
///
/// **C'est la clause dont le score dépend entièrement** : un coup qui paierait deux
/// fois paierait un démon deux fois, et un coup qui paierait avant le dernier paierait
/// des créatures encore debout. Les trois régimes y sont — les coups qui entament,
/// celui qui abat, et ceux qui tombent sur une morte.
///
/// **Les coups d'après comptent dans le cas réel** : une créature joue sa chute une
/// seconde et deux dixièmes avant de quitter la population, et une rafale la traverse.
/// Son volume est retiré du tir pendant ce temps, mais c'est une règle de la partie,
/// et celle-ci ne doit pas être la seule garde.
///
/// **Les trois silhouettes y passent**, et la prime rendue se compare à celle de leur
/// ligne : un retour pris dans la mauvaise ligne de la table paierait toujours la
/// même, ce qu'un corpus d'une seule créature ne verrait pas.
#[test]
fn un_seul_coup_est_paye() {
    let (grid, map) = maze(SEEDS[0]);
    let push = Vec3::new(1.0, 0.0, 0.0);

    for figure in &FIGURES {
        let mut monster = lone(&grid, &map, plain_case(&grid));
        monster.figure = figure;

        for coup in 1..LIFE {
            assert_eq!(
                monster.knock(push),
                None,
                "{} : le coup {coup} a payé une créature qui encaisse {LIFE} coups",
                figure.name
            );
        }

        assert_eq!(
            monster.knock(push),
            Some(figure.bounty),
            "{} : le coup fatal n'a pas rendu sa prime",
            figure.name
        );

        for coup in 1..=3 {
            assert_eq!(
                monster.knock(push),
                None,
                "{} : le coup {coup} après la chute l'a payée une seconde fois",
                figure.name
            );
        }
    }
}

/// Les trois primes sont distinctes.
///
/// **Sans quoi le compteur ne dirait pas laquelle on a abattue**, et c'est la seule
/// raison pour laquelle elles diffèrent aujourd'hui : rien ne rend encore une
/// silhouette plus difficile qu'une autre, et l'ordre des trois valeurs se reprendra
/// quand leur résistance divergera. Ce qui ne doit pas se perdre dans cette reprise
/// est qu'elles restent trois.
#[test]
fn les_trois_primes_sont_distinctes() {
    let mut bounties: Vec<u32> = FIGURES.iter().map(|figure| figure.bounty).collect();
    let before = bounties.len();
    bounties.sort_unstable();
    bounties.dedup();

    assert_eq!(
        bounties.len(),
        before,
        "deux silhouettes partagent une prime : {:?}",
        FIGURES
            .iter()
            .map(|figure| (figure.name, figure.bounty))
            .collect::<Vec<_>>()
    );
}

/// Une créature tombée ne marche plus, mais son cycle avance et elle tombe encore.
///
/// **Trois propriétés qui se tiennent** : sans la première, un cadavre se promène ;
/// sans la deuxième, il reste figé sur sa première trame, puisque la phase de mort
/// n'avance ni avec la distance — il n'en parcourt aucune — ni toute seule ; sans la
/// troisième, un démon abattu au bord d'un palier resterait en l'air.
#[test]
fn une_creature_tombee_ne_marche_plus() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monster = lone(&grid, &map, plain_case(&grid));
    for _ in 0..LIFE {
        monster.knock(Vec3::new(1.0, 0.0, 0.0));
    }

    let before = monster.at();
    for _ in 0..60 {
        monster.walk(&map, DT, false);
    }

    let gone = monster.at() - before;
    let flat = Vec3::new(gone.x, gone.y, 0.0);
    assert!(
        flat.dot(flat).sqrt() <= 0.01,
        "la créature tombée a parcouru {} à l'horizontale",
        flat.dot(flat).sqrt()
    );
    assert!(
        monster.phase > 0.0,
        "le cycle de mort n'a pas avancé en une seconde"
    );
    assert_eq!(monster.motion, Motion::Dead, "elle a changé de cycle");
}

/// Le cycle de mort s'arrête sur sa dernière trame et n'y revient pas.
///
/// **C'est ce qui distingue la mort des deux autres cycles**, et la planche le porte
/// déjà : seize trames au lieu de huit, parce qu'elle doit tenir entière plutôt que de
/// se boucler. Une mort qui recommencerait ferait se relever le démon.
#[test]
fn le_cycle_de_mort_garde_sa_derniere_trame() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monster = lone(&grid, &map, plain_case(&grid));
    for _ in 0..LIFE {
        monster.knock(Vec3::new(1.0, 0.0, 0.0));
    }

    // Dix fois la durée du cycle : largement de quoi reboucler si rien ne l'en
    // empêchait.
    let last = sheet::column(Motion::Dead, 1.0);
    for _ in 0..(10.0 * DEAD_PERIOD / DT) as usize {
        monster.walk(&map, DT, false);
    }

    assert_eq!(
        sheet::column(Motion::Dead, monster.phase),
        last,
        "après dix cycles, la mort est revenue à la trame {} au lieu de rester \
         sur la dernière",
        sheet::column(Motion::Dead, monster.phase)
    );
}

/// Une créature abattue finit par ne plus rien laisser.
///
/// **Deux temps, et le second est une conséquence du premier** : elle est consommée
/// quand son cycle de mort est joué en entier — c'est le seul moment où la silhouette
/// se donne à voir, et l'effacer avant perdrait la planche —, et elle ne l'est pas
/// avant, sinon le corps disparaîtrait en pleine chute.
#[test]
fn une_creature_abattue_finit_par_s_effacer() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monster = lone(&grid, &map, plain_case(&grid));
    for _ in 0..LIFE {
        monster.knock(Vec3::new(1.0, 0.0, 0.0));
    }
    assert!(
        !monster.spent(),
        "la créature est effacée avant d'avoir joué sa chute"
    );

    // À une image de la fin du cycle, elle est encore là.
    let frames = (DEAD_PERIOD / DT).ceil() as usize;
    for _ in 0..frames - 1 {
        monster.walk(&map, DT, false);
    }
    assert!(
        !monster.spent(),
        "la chute s'interrompt avant ses {DEAD_PERIOD} secondes"
    );

    monster.walk(&map, DT, false);
    assert!(
        monster.spent(),
        "la chute est jouée et la créature reste, à la phase {}",
        monster.phase
    );
}

/// Deux créatures enchevêtrées finissent par se séparer.
///
/// **Le corpus commence en recouvrement**, et c'est tout ce qui la distingue de
/// `deux_demons_ne_se_marchent_pas_dessus` : celle-là part de poses qui ne se recouvrent
/// jamais, donc elle ne voit pas l'état bloqué — elle vérifie qu'on n'y entre pas, pas
/// qu'on en sort. Le recul, lui, y fait entrer, puisqu'il ignore l'évitement.
///
/// **Deux caps sont éprouvés, et le second est le cas dégénéré** : opposés, les pas
/// éloignent donc le dégagement est immédiat ; identiques, aucun pas n'éloigne jamais —
/// elles avancent du même écart — et ce qui les sépare est la patience, qui en retourne
/// une. Sans ce second cas, un correctif qui ne traiterait que l'évident passerait.
#[test]
fn deux_creatures_enchevetrees_se_separent() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;
    /// Combien d'images elles ont pour se dégager.
    ///
    /// Trois secondes : la patience en coûte douze au pire, et le reste est la marche
    /// qui écarte. Ce qui est cherché est un blocage, pas une cadence.
    const FRAMES: usize = 180;

    for (seed, caps) in [
        (SEEDS[0], [0.0, core::f32::consts::PI]),
        (SEEDS[4], [0.0, 0.0]),
    ] {
        let (grid, map) = maze(seed);
        let at = plain_case(&grid);

        // Sur la même case, donc au même centre : le recouvrement est total, ce qui est
        // pire que ce qu'un recul produit.
        let mut crowd = vec![lone(&grid, &map, at), lone(&grid, &map, at)];
        crowd[0].facing = caps[0];
        crowd[1].facing = caps[1];
        assert!(
            meets(crowd[0].at(), crowd[1].at()),
            "graine {seed:#x} : les deux créatures ne partent pas enchevêtrées"
        );

        let mut freed = None;
        for frame in 0..FRAMES {
            stroll(&mut crowd, &map, DT);
            if !meets(crowd[0].at(), crowd[1].at()) {
                freed = Some(frame);
                break;
            }
        }

        assert!(
            freed.is_some(),
            "graine {seed:#x}, caps {caps:?} : après {FRAMES} images, les deux \
             créatures sont toujours enchevêtrées, en {:?} et {:?}",
            crowd[0].at(),
            crowd[1].at()
        );
    }
}

/// Deux créatures ne se marchent pas dessus.
///
/// **Le moteur ne l'empêche pas et ne le peut pas** : il n'arrête que la géométrie
/// de cellule, et un démon n'a ni portail ni adjacence. Sans la règle du jeu, deux
/// silhouettes se traversent — ce qui se voit, et se voit mal.
///
/// **Elle part du régime réel**, sur une marche de quinze secondes : les trois
/// naissent sur des cases voisines du départ et se croisent donc dans les premières
/// secondes, ce qu'un départ écarté n'éprouverait pas.
///
/// **Ce qu'elle ne couvre pas, et c'est nommé** : le joueur. Le contact entre lui et
/// une créature est une règle de l'étape 4, et rien ici ne l'anticipe.
#[test]
fn deux_demons_ne_se_marchent_pas_dessus() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;
    /// Combien d'images elles marchent.
    const FRAMES: usize = 900;

    for seed in [SEEDS[0], SEEDS[4]] {
        let (grid, map) = maze(seed);
        let mut monsters = population(&grid, &map).expect("planches du dépôt valides");

        for frame in 0..FRAMES {
            stroll(&mut monsters, &map, DT);

            for (rank, monster) in monsters.iter().enumerate() {
                for other in monsters.iter().skip(rank + 1) {
                    assert!(
                        !meets(monster.at(), other.at()),
                        "graine {seed:#x} : à l'image {frame}, {} et {} occupent le \
                         même volume, en {:?} et {:?}",
                        monster.name(),
                        other.name(),
                        monster.at(),
                        other.at()
                    );
                }
            }
        }
    }
}

/// Une créature gênée par une autre finit par se détourner.
///
/// **C'est le pendant de la règle précédente**, et sans lui elle serait un piège :
/// deux créatures qui s'arrêtent l'une devant l'autre sans jamais se détourner
/// resteraient face à face indéfiniment, ce qui est pire que de se traverser.
///
/// **Ce qui le donne est que le pas demandé reste celui du cap**, même annulé : le
/// critère d'arrêt compare ce qui a été franchi à ce qui était demandé, donc une
/// image gênée par une autre créature s'épuise comme une image contre un mur. Comparé
/// au pas réellement tenté, qui est nul, un pas nul paraîtrait franchi en entier et
/// la patience ne s'épuiserait jamais.
#[test]
fn une_creature_genee_par_une_autre_se_detourne() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");
    let first = monsters[0].facing;

    // Gênée à chaque image, sans qu'aucun décor soit en cause.
    for _ in 0..PATIENCE {
        monsters[0].walk(&map, DT, true);
    }

    assert_ne!(
        monsters[0].facing, first,
        "gênée pendant {PATIENCE} images, la créature garde son cap {first} : \
         deux silhouettes se feraient face indéfiniment"
    );
}

/// Le décor finit par l'arrêter, et elle fait demi-tour.
///
/// **Le critère porte sur la distance et non sur un axe**, et c'est ce que cette
/// épreuve paie : le balayage rend la surface de moindre pénétration, donc une
/// créature poussée contre un mur peut se voir arrêtée par le sol. Conditionné à
/// l'axe du cap, le demi-tour ne se déclencherait jamais — elle dériverait le long
/// de la paroi en avançant toujours, et cette épreuve le dirait.
///
/// **Elle marche seule, sans foule**, et c'est voulu : ce qu'on mesure est ce que le
/// décor impose, et un demi-tour dû à une autre créature le masquerait.
///
/// **Ce qui est exigé est qu'elle reparte**, pas seulement qu'elle tourne : un cap
/// inversé qui laisserait la créature collée au mur serait un demi-tour pour rien.
#[test]
fn le_decor_retourne_le_demon() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;
    /// Combien d'images on lui laisse pour buter puis repartir.
    const FRAMES: usize = 600;

    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");
    let monster = &mut monsters[0];
    let first = monster.facing;
    let mut turns = 0;
    let mut farthest: f32 = 0.0;

    let origin = monster.at();
    for _ in 0..FRAMES {
        let before = monster.facing;
        monster.walk(&map, DT, false);
        if monster.facing != before {
            turns += 1;
        }
        let gap = monster.at() - origin;
        farthest = farthest.max(gap.dot(gap).sqrt());
    }

    assert!(
        turns > 0,
        "en {FRAMES} images, le démon n'a jamais fait demi-tour : parti au cap \
         {first}, il y est encore"
    );
    assert!(
        farthest > export::CELL,
        "le démon n'a jamais dépassé {farthest} du départ : il tourne sur place \
         au lieu de repartir"
    );
}

/// Un cap oblique ne laisse pas la créature dériver le long d'un mur.
///
/// **C'est elle qui paie le seuil de [`STALLED`]**, et aucune autre : poussée de
/// travers contre une paroi, la créature garde la composante tangentielle de son
/// pas — `cos 45°`, soit `0,707` — donc elle avance, mais le long du mur et non vers
/// son cap. À un seuil de `0,5`, elle longe indéfiniment sans jamais être retournée,
/// et c'est ce que cette épreuve a relevé : neuf cents images sans un seul
/// demi-tour.
///
/// **Ce qu'elle ne paie pas, et qu'il faut savoir** : le choix de porter le critère
/// sur la **distance** plutôt que sur l'axe du cap. Remplacé par un test du seul
/// axe `X`, les trois épreuves de marche restent vertes — ce décor finit toujours
/// par présenter une paroi perpendiculaire à `X`, si bien que les deux critères se
/// déclenchent tous deux, plus tard pour l'un que pour l'autre. Le choix reste le
/// juste, le balayage rendant la surface de moindre pénétration, mais il est tenu
/// par le raisonnement et non par une épreuve.
///
/// Ce qui est exigé est qu'elle **s'arrête de dériver** : sur une longue course, la
/// distance au point de départ reste bornée, là où une dérive non corrigée
/// l'emmènerait au bout du couloir.
#[test]
fn un_cap_oblique_ne_fait_pas_deriver() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;
    /// Combien d'images la créature marche en biais.
    const FRAMES: usize = 900;

    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");
    let monster = &mut monsters[0];
    // En biais sur les deux axes : aucune composante ne s'annule, donc le décor
    // l'arrête toujours par l'un sans l'arrêter par l'autre.
    monster.facing = core::f32::consts::FRAC_PI_4;

    let origin = monster.at();
    let mut turns = 0;
    let mut farthest: f32 = 0.0;

    for _ in 0..FRAMES {
        let before = monster.facing;
        monster.walk(&map, DT, false);
        if monster.facing != before {
            turns += 1;
        }
        let gap = monster.at() - origin;
        farthest = farthest.max(gap.dot(gap).sqrt());
    }

    assert!(
        turns > 0,
        "parti en biais, le démon n'a jamais fait demi-tour en {FRAMES} images"
    );
    // Ce qu'une marche de quinze secondes couvrirait sans jamais être retournée :
    // la borne est large, et c'est une dérive qu'on cherche, pas une cadence.
    let loose = SPEED * DT * FRAMES as f32 / 2.0;
    assert!(
        farthest < loose,
        "parti en biais, le démon s'est éloigné de {farthest} du départ : \
         il dérive le long des parois au lieu d'être retourné"
    );
}

/// Pose les quatre vues cardinales des trois silhouettes dans `.tmp/`, pour qu'on
/// les regarde.
///
/// **C'est un instrument et non une épreuve**, et il n'affirme rien : de quel côté
/// une silhouette regarde ne s'écrit dans aucune assertion, puisque les trois
/// démons sont trop symétriques pour qu'un centre de masse décide — mesuré, l'écart
/// tête-bassin change de signe d'une silhouette à l'autre.
///
/// **Et c'est lui qui a tranché le sens de rotation des vues.** À cap nul et œil
/// posé en `+Y`, la caméra regarde le `−Y` : sa droite d'image est donc le `−X`, et
/// une créature qui regarde le `+X` doit apparaître tournée vers la **gauche**.
/// Elle pointait vers la droite, ce qui a nommé le défaut — les vues étaient lues à
/// l'envers, et la créature marchait à reculons. Les vues de face et de dos,
/// rendues du même geste, ont écarté l'autre explication possible : un décalage
/// d'un demi-tour les aurait échangées, et elles sont justes.
///
/// **Les trois silhouettes et non la première** : rien ne garantit que la chaîne ait
/// rangé les lignes des trois planches dans le même ordre, et c'est exactement le
/// genre de convention qu'aucun octet ne publie.
#[test]
fn image_des_quatre_vues() {
    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");

    for monster in &mut monsters {
        monster.motion = Motion::Walk;
        monster.phase = 2.5 / 8.0;
        monster.facing = 0.0;

        let centre = monster.at();
        // Quatre points cardinaux autour d'elle, cap nul : `relative` vaut donc
        // l'angle du point, et la vue attendue en découle — 0 de face, 2 et 6 de
        // profil, 4 de dos.
        for (name, turn) in [
            ("vue-devant", 0.0),
            ("vue-flanc-plus-y", core::f32::consts::FRAC_PI_2),
            ("vue-derriere", core::f32::consts::PI),
            ("vue-flanc-moins-y", -core::f32::consts::FRAC_PI_2),
        ] {
            let mut context = context(0);
            let (sin, cos) = turn.sin_cos();
            let mut eye = FreeCamera::new(Vec3::new(
                centre.x + 1.6 * cos,
                centre.y + 1.6 * sin,
                centre.z + 0.3,
            ));
            // On regarde vers elle : le cap de l'œil est l'opposé de sa direction.
            eye.yaw = turn + core::f32::consts::PI;
            context.set_camera(eye.camera()).expect("pose tenable");
            submit(&mut context, core::slice::from_ref(monster), &eye.camera())
                .expect("créature soumise");
            snapshot(&format!("{name}-{}", monster.name()), &frame(&mut context));
        }
    }
}

/// Pose dans `.tmp/` chaque silhouette au ras du sol, décor compris, pour juger du
/// contact.
///
/// **Le contact ne se lit pas dans une assertion** : l'ancrage peut être
/// arithmétiquement juste et la créature flotter quand même, parce que la
/// demi-hauteur du **corps** ne vaut pas celle du **dessin**. Seule une image au
/// ras du sol le montre, et c'est pour cela que la caméra est posée à hauteur de
/// pied plutôt qu'à hauteur d'œil.
///
/// **Les trois et non la première, et c'est ce lot qui l'impose** : la marge de
/// cadrage vient de la table, donc un contact juste pour une silhouette ne dit rien
/// des deux autres.
#[test]
fn image_du_contact() {
    let scenery = Scenery::new(Settings {
        extent: (16, 16, 2),
        seed: SEEDS[0],
        stairs: 6,
        ramps: 2,
        loops: 8,
    })
    .expect("labyrinthe et planches valides");
    let mut monsters = population(&scenery.maze, &scenery.map).expect("planches du dépôt valides");

    for monster in &mut monsters {
        monster.motion = Motion::Walk;
        monster.phase = 2.5 / 8.0;

        let centre = monster.at();
        let foot = centre.z - HALF.z;

        // Deux cadrages : l'un à hauteur de genou pour la silhouette entière,
        // l'autre au ras du sol et tout près, où un écart d'un texel occupe
        // plusieurs pixels.
        for (name, back, height) in [
            ("contact", 2.0, foot + 0.35),
            ("contact-pres", 0.9, foot + 0.1),
        ] {
            let mut context = context(0);
            let mut eye = FreeCamera::new(Vec3::new(centre.x, centre.y + back, height));
            eye.yaw = -core::f32::consts::FRAC_PI_2;
            let view = View {
                camera: eye.camera(),
                cell: monster.body.cell(),
            };

            crate::scene::submit(&mut context, &scenery, &view).expect("décor soumis");
            submit(&mut context, core::slice::from_ref(monster), &view.camera)
                .expect("créature soumise");
            snapshot(&format!("{name}-{}", monster.name()), &frame(&mut context));
        }
    }
}

/// La tache d'ombre couvre l'empreinte des pieds de toutes les silhouettes.
///
/// **Elle est née d'un flottement qui n'en était pas un.** La créature paraissait
/// en l'air ; la mesure dans l'image rendue — caméra posée à la cote du sol, donc
/// sol projeté sur la ligne d'horizon — montrait le bas du dessin exactement sur
/// cette ligne, et parfois un pixel dessous. Le dessin ne flottait donc pas : c'est
/// l'ombre qui était trop étroite, et des pieds qui dépassent de leur ombre se
/// lisent comme des pieds en l'air.
///
/// **Les six planches, parce que la tache est unique** : taillée sur la plus large
/// des trois silhouettes, elle couvre les deux autres de surcroît, et c'est la plus
/// large qui la dimensionne. Une planche refaite qui chausserait plus grand doit
/// faire rougir ici.
#[test]
fn la_tache_couvre_l_empreinte_des_pieds() {
    let side = FRAME as u32;
    let mut widest = 0;
    let mut owner = "";

    for figure in &FIGURES {
        for bytes in [figure.idle, figure.walk] {
            let sheet = load_png_masked(bytes).expect("planche du dépôt valide");

            for view in 0..(sheet.height() / side) {
                for frame in 0..(sheet.width() / side) {
                    let span = footprint(&sheet, view, frame);
                    if span > widest {
                        widest = span;
                        owner = figure.name;
                    }
                }
            }
        }
    }

    let footprint = widest as f32 / FRAME * (2.0 * SPRITE_HALF);
    assert!(
        2.0 * SHADOW_RADIUS >= footprint,
        "l'empreinte des pieds de {owner} fait {footprint} de large et la tache {} \
         de diamètre : les pieds en dépassent",
        2.0 * SHADOW_RADIUS
    );
}

/// La patience dépasse ce que le décor produit d'accrocs en marche normale.
///
/// **C'est ce qui sépare un obstacle d'un cahot**, et le réglage s'est trouvé faux
/// à l'écran : la créature pivotait en ligne droite. Mesuré, le décor produit des
/// séries d'images gênées qui vont jusqu'à six — un joint de dalle, la reprise
/// d'une pente —, et la patience valait six. Elle se déclenchait donc sur le cahot
/// le plus long que la marche normale produise.
///
/// **La mesure se prend sans demi-tour**, en passant par le corps directement : la
/// politique de marche remet le compteur à zéro en se retournant, donc elle
/// tronquerait les séries qu'on cherche à mesurer.
#[test]
fn la_patience_depasse_les_accrocs_du_decor() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;
    /// Combien d'images on marche pour relever les séries.
    const FRAMES: usize = 900;

    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");
    let monster = &mut monsters[0];
    let step = monster.step(DT);

    let (mut worst, mut run) = (0u32, 0u32);
    for _ in 0..FRAMES {
        let before = monster.at();
        monster.body.advance(&map, step, DT);
        let after = monster.at();

        let gone = Vec3::new(after.x - before.x, after.y - before.y, 0.0);
        // Une fois contre le mur du fond, elle y reste : on ne compte donc que
        // les séries qui se terminent, c'est-à-dire les cahots.
        match gone.dot(gone) < step.dot(step) * STALLED * STALLED {
            true => run += 1,
            false => {
                worst = worst.max(run);
                run = 0;
            }
        }
    }

    assert!(
        PATIENCE > worst,
        "le décor produit des séries de {worst} images gênées sans qu'aucun mur \
         soit en cause, et la patience vaut {PATIENCE} : la créature pivote sur \
         un cahot"
    );
}

/// La marge annoncée est la plus fréquente de la planche du cycle.
///
/// **Née d'un symptôme d'écran, et corrigée deux fois.** La créature a d'abord paru
/// sauter sur place : la mesure a écarté la cause supposée — au repos, le bas ne
/// bouge d'aucun texel — et donné trois au lieu de deux. Puis elle s'est mise à
/// marcher et a flotté, et le minimum du cycle a paru la réponse : il était pire.
///
/// **C'est le mode qui pose le sol, et non le minimum.** La marche de `d1` relève
/// `[3, 4, 4, 4, 4, 4, 3, 2]` : ancrée sur le `2`, la créature flotte d'un à deux
/// texels pendant **sept trames sur huit**, alors que la trame la plus basse est
/// l'exception. Ancrée sur le `4`, cinq trames touchent exactement et les deux
/// autres s'enfoncent d'un texel — ce qui se voit infiniment moins qu'un pied en
/// l'air.
///
/// **Les six valeurs sont six mesures distinctes**, et c'est ce que la population a
/// appris : `3/4`, `2/3` et `3/2` selon la silhouette et le cycle. Aucune ne se
/// déduit d'une autre, donc une table partagée aurait fait flotter cinq cadrages sur
/// six.
///
/// **Le repos n'oscille pas**, et c'est le second contrôle : chaque planche de repos
/// donne la même marge sur ses huit trames. Une planche dont elle varierait ferait
/// sautiller une créature immobile, et ce serait cette fois un vrai défaut d'asset.
#[test]
fn la_marge_annoncee_est_la_plus_frequente() {
    let side = FRAME as u32;

    for figure in &FIGURES {
        // **Les trois cycles, et le second temps n'en couvre qu'un** : la mort y
        // échapperait de toute façon, une créature qui tombe changeant de hauteur à
        // chaque trame. C'est donc le mode seul qui l'ancre, comme pour la marche.
        for (bytes, motion) in [
            (figure.idle, Motion::Idle),
            (figure.walk, Motion::Walk),
            (figure.dead, Motion::Dead),
        ] {
            let sheet = load_png_masked(bytes).expect("planche du dépôt valide");
            let (views, frames) = (sheet.height() / side, sheet.width() / side);

            // Le compte de chaque marge rencontrée, indexé par sa valeur en texels.
            let mut tally = vec![0u32; side as usize + 1];
            for view in 0..views {
                for frame in 0..frames {
                    tally[hollow(&sheet, view, frame) as usize] += 1;
                }
            }
            let common = tally
                .iter()
                .enumerate()
                .max_by_key(|&(_, count)| count)
                .map(|(value, _)| value as f32)
                .unwrap_or_default();

            assert_eq!(
                figure.margin(motion),
                common,
                "{} en {motion:?} : la marge la plus fréquente est {common}, et la \
                 table en annonce {}",
                figure.name,
                figure.margin(motion)
            );
        }

        let idle = load_png_masked(figure.idle).expect("planche du dépôt valide");
        let (views, frames) = (idle.height() / side, idle.width() / side);
        for view in 0..views {
            for frame in 0..frames {
                assert_eq!(
                    hollow(&idle, view, frame) as f32,
                    figure.margin(Motion::Idle),
                    "{} au repos : la vue {view}, trame {frame}, laisse {} texel(s), \
                     et une créature immobile sautillerait",
                    figure.name,
                    hollow(&idle, view, frame)
                );
            }
        }
    }
}

/// La foulée annoncée est celle que la planche dessine.
///
/// **Elle se relève sur les deux vues de profil** : l'empreinte des pieds y passe de
/// son écartement le plus serré au plus large, et ce que le cycle gagne entre les deux
/// est la distance entre deux appuis. Le cycle porte **deux pas** — ses huit trames
/// montrent deux maxima —, donc la foulée vaut deux fois cet écart.
///
/// **Les trois ne s'accordent pas, et c'est la trouvaille de ce lot** : trente-six
/// texels pour `d1`, seize pour `d2`, vingt-deux pour `d3`. Partagée, la foulée de
/// `d1` ferait avancer le cycle de `d2` deux fois et quart trop lentement pour la
/// distance parcourue, et ses pieds patineraient — exactement le défaut que la mesure
/// de `d1` avait corrigé, mais sur deux silhouettes au lieu d'une.
///
/// **À l'égalité exacte**, les deux membres étant des comptes de texels : la
/// conversion en unités de monde n'intervient qu'au moment de l'emploi.
#[test]
fn la_foulee_annoncee_est_celle_de_la_planche() {
    let side = FRAME as u32;

    for figure in &FIGURES {
        let sheet = load_png_masked(figure.walk).expect("planche du dépôt valide");
        let frames = sheet.width() / side;

        let (mut tight, mut wide) = (side, 0);
        for view in PROFILES {
            for frame in 0..frames {
                let span = footprint(&sheet, view, frame);
                tight = tight.min(span);
                wide = wide.max(span);
            }
        }

        let measured = 2.0 * (wide - tight) as f32;
        assert_eq!(
            figure.stride, measured,
            "{} : de profil, l'empreinte passe de {tight} à {wide} texels, soit une \
             foulée de {measured}, et la table en annonce {}",
            figure.name, figure.stride
        );
    }
}

/// Le cycle de repos fait un tour en sa période.
///
/// **Il avance avec l'horloge, et c'est ce qui le sépare d'une marche** : une
/// créature immobile n'a aucune distance à offrir, donc un cycle indexé sur le
/// déplacement resterait figé sur sa première trame. L'épreuve enchaîne des pas
/// d'image plutôt qu'un saut unique, qui ne dirait rien de l'accumulation.
#[test]
fn le_cycle_de_repos_tourne_avec_le_temps() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");
    let monster = &mut monsters[0];
    assert_eq!(monster.phase, 0.0);

    let frames = (IDLE_PERIOD / DT).round() as u32;
    for _ in 0..frames {
        monster.advance(0.0, DT);
    }

    assert_eq!(monster.motion, Motion::Idle, "le repos a changé de cycle");
    assert!(
        (monster.phase - 1.0).abs() < 0.01,
        "après {IDLE_PERIOD} seconde(s), le cycle de repos en est à {} tour(s)",
        monster.phase
    );
}

/// Une distance parcourue fait passer à la marche, et le temps n'y suffit pas.
///
/// **Une créature qui marcherait en montrant des poses de repos ne se verrait pas
/// tout de suite**, et c'est ce que cette règle évite : le cycle de marche est indexé
/// sur ce que le décor a laissé parcourir, et la foulée est celle de la silhouette.
///
/// **La phase repart de zéro au changement**, les deux cycles n'ayant pas la même
/// foulée : reportée, elle tomberait au milieu d'un pas.
#[test]
fn une_distance_parcourue_fait_passer_a_la_marche() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monsters = population(&grid, &map).expect("planches du dépôt valides");

    // Les trois, parce que la foulée vient de la table : une phase calculée sur la
    // foulée d'une autre silhouette avancerait du double sans qu'aucune épreuve de
    // `d1` le dise.
    for monster in &mut monsters {
        // Quelques images de repos, de quoi charger la phase.
        for _ in 0..30 {
            monster.advance(0.0, DT);
        }
        assert!(monster.phase > 0.0, "le repos n'a pas avancé sa phase");

        monster.advance(monster.figure.stride() / 4.0, DT);
        assert_eq!(
            monster.motion,
            Motion::Walk,
            "{} : une distance parcourue n'a pas fait passer à la marche",
            monster.name()
        );
        assert!(
            (monster.phase - 0.25).abs() < 1.0e-6,
            "{} : un quart de foulée met le cycle de marche à {} tour",
            monster.name(),
            monster.phase
        );

        // Et le retour au repos : la phase repart de zéro, elle ne se reporte pas.
        monster.advance(0.0, DT);
        assert_eq!(monster.motion, Motion::Idle);
        assert!(
            (monster.phase - DT / IDLE_PERIOD).abs() < 1.0e-6,
            "{} : au retour au repos, la phase est à {} au lieu de repartir de zéro",
            monster.name(),
            monster.phase
        );
    }
}

/// Une dalle unie sous la caméra, de quoi lire un facteur de modulation.
///
/// **Un cas minimal et non la scène du jeu**, et c'est ce qu'une mesure demande :
/// sous la scène, le pixel qu'on vise porte une texture de sol, un angle
/// d'éclairage et peut-être le sprite de la créature. Ici il ne porte qu'un gris
/// connu, donc ce qu'on lit est le facteur et rien d'autre.
fn slab() -> Texture {
    let side = 4u32;
    let bytes = vec![0x80; (side * side) as usize * 4];

    Texture::load(side, side, &bytes)
        .unwrap_or_else(|_| unreachable!("carrée et puissance de deux"))
}

/// Le pixel du centre de l'image, en RGB.
fn middle(pixels: &[u8]) -> [u8; 3] {
    let at = ((HEIGHT / 2) * WIDTH + WIDTH / 2) as usize * BYTES_PER_PIXEL;

    [pixels[at], pixels[at + 1], pixels[at + 2]]
}

/// Soumet une dalle unie vue de dessus, et rend l'image.
///
/// La caméra plonge à la verticale sur l'origine : c'est l'angle sous lequel une
/// tache au sol se lit entière, et il ne demande aucune rotation à écrire à la main
/// — `FreeCamera` compose le tangage dans le bon ordre.
fn over_slab(shade: bool) -> Vec<u8> {
    let mut context = context(0);
    let mut eye = FreeCamera::new(Vec3::new(0.0, 0.0, 3.0));
    eye.pitch = -core::f32::consts::FRAC_PI_2;
    context.set_camera(eye.camera()).expect("pose tenable");

    let ground = Arc::new(slab());
    let reach = 4.0;
    let corners = [
        Vec3::new(-reach, -reach, 0.0),
        Vec3::new(reach, -reach, 0.0),
        Vec3::new(reach, reach, 0.0),
        Vec3::new(-reach, reach, 0.0),
    ];
    let white = Color::new(0xFF, 0xFF, 0xFF, 0xFF);
    let vertices: Vec<VertexUv> = corners
        .iter()
        .zip([(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)])
        .map(|(&position, (u, v))| VertexUv { position, u, v })
        .collect();
    let faces = [
        Triangle {
            indices: [0, 1, 2],
            color: white,
        },
        Triangle {
            indices: [0, 2, 3],
            color: white,
        },
    ];

    context
        .submit_textured(Affine3::IDENTITY, &vertices, &faces, &ground)
        .expect("dalle soumise");

    if shade {
        let shadow = Arc::new(shadow_texture());
        let spot = shadow_corners(Vec3::new(0.0, 0.0, HALF.z));
        let marks: Vec<VertexUv> = spot
            .iter()
            .zip([(0.0, 0.0), (SIDE, 0.0), (SIDE, SIDE), (0.0, SIDE)])
            .map(|(&position, (u, v))| VertexUv { position, u, v })
            .collect();
        context
            .submit_blended(Affine3::IDENTITY, &marks, &faces, Some(&shadow))
            .expect("tache soumise");
    }

    frame(&mut context)
}

/// Le facteur d'une surface modulée s'applique entier sur le chemin non éclairé.
///
/// **C'est la mesure que ce lot existe pour prendre, et elle ne se reprendra pas.**
/// Dès qu'une lumière dynamique sera réglée à l'étape 8, toute soumission passera
/// par le chemin éclairé : le chemin nu disparaîtra comme cas observable, et avec
/// lui la seule référence contre laquelle comparer. Le soupçon qui la motive est que
/// le facteur soit multiplié par l'éclairage reçu — auquel cas une tache hors de
/// portée de toute lampe noircirait la dalle au lieu de l'assombrir.
///
/// **Elle attrape aussi le sens des sommets**, et c'est ce qui la rend doublement
/// utile : décrit à l'envers, le quadrilatère est un dos de face et disparaît sans
/// lever d'erreur. Le pixel resterait alors identique, et la première assertion le
/// dit.
///
/// La mesure : dalle unie à `0x80`, tache dont le centre vaut `0x38`. Le produit
/// attendu est `128 × 56 / 255`, soit `28` — à une unité près, le moteur modulant en
/// entiers.
#[test]
fn le_facteur_de_modulation_s_applique_entier_sans_eclairage() {
    /// Ce que la dalle vaut avant la tache, sur 255.
    const GROUND: f32 = 0x80 as f32;

    let bare = middle(&over_slab(false));
    let shaded = middle(&over_slab(true));

    assert_ne!(
        bare, shaded,
        "la tache n'a rien changé au sol : soit elle n'est pas soumise, \
         soit ses sommets sont décrits dans le sens qui la rend invisible"
    );

    let expected = (GROUND * SHADOW_CORE / 255.0).round();
    for (channel, (&got, &was)) in shaded.iter().zip(bare.iter()).enumerate() {
        assert!(
            got < was,
            "canal {channel} : la tache a éclairci le sol, de {was} à {got}, \
             alors qu'une surface modulée n'éclaircit jamais"
        );
        assert!(
            (got as f32 - expected).abs() <= 1.0,
            "canal {channel} : la dalle à {was} sous une tache à {SHADOW_CORE} \
             rend {got}, là où le facteur entier donnerait {expected}"
        );
    }
}

/// Ce qu'on voit d'une créature tient dans ce qu'on peut toucher.
///
/// **C'est la propriété qui manquait**, et son absence s'est vue à l'écran : viser le
/// corps d'une silhouette dessinée plus large que son volume rate le coup, et rien ne
/// dit pourquoi. La largeur dessinée se relève sur les planches, la largeur touchable
/// vient du volume que le tir reçoit, et les deux doivent s'accorder.
///
/// **Le cas défavorable est la vue de face**, et c'est lui qu'on prend : le volume est
/// une boîte alignée sur les axes du monde, donc sa largeur apparente va de son côté à
/// sa diagonale selon l'angle sous lequel on la regarde. Un dessin qui tient dans le
/// côté tient dans tous les angles.
///
/// **Les deux cycles vivants seulement** : une créature tombée n'est plus touchable, et
/// sa planche la couche — sa largeur n'a donc rien à accorder.
#[test]
fn le_dessin_tient_dans_le_volume_touchable() {
    let side = FRAME as u32;
    let texel = 2.0 * SPRITE_HALF / FRAME;

    let mut report = Vec::new();
    for figure in &FIGURES {
        for bytes in [figure.idle, figure.walk] {
            let sheet = load_png_masked(bytes).expect("planche du dépôt valide");
            let (views, frames) = (sheet.height() / side, sheet.width() / side);

            let poses: Vec<(u32, u32)> = (0..views)
                .flat_map(|view| (0..frames).map(move |frame| (view, frame)))
                .collect();
            let hull = poses
                .iter()
                .map(|&(view, frame)| drawn_width(&sheet, view, frame))
                .max()
                .expect("une planche a des vignettes");
            let trunk = poses
                .iter()
                .map(|&(view, frame)| trunk_width(&sheet, view, frame))
                .max()
                .expect("une planche a des vignettes");

            let rise = poses
                .iter()
                .map(|&(view, frame)| {
                    side - hollow(&sheet, view, frame) - crown(&sheet, view, frame)
                })
                .max()
                .expect("une planche a des vignettes");

            report.push((
                figure.name,
                hull,
                hull as f32 * texel,
                trunk,
                trunk as f32 * texel,
                rise,
                rise as f32 * texel,
            ));
        }
    }

    for (name, hull, wide, trunk, body, rise, tall) in report {
        let figure = FIGURES
            .iter()
            .find(|figure| figure.name == name)
            .expect("une ligne par nom relevé");
        let touch = figure.touch();

        assert!(
            body <= 2.0 * touch.x,
            "{name} : son corps est dessiné sur {trunk} texels, soit {body:.2} de \
             large, et son volume touchable n'en fait que {:.2} — viser son flanc \
             raterait",
            2.0 * touch.x
        );
        assert!(
            tall <= 2.0 * touch.z,
            "{name} : elle est dessinée sur {rise} texels, soit {tall:.2} de haut, et \
             son volume touchable n'en fait que {:.2} — viser sa tête raterait",
            2.0 * touch.z
        );

        // L'enveloppe, elle, **dépasse** et c'est voulu : un volume qui la couvrirait
        // rendrait touchable le vide entre les membres. Ce qui est vérifié est qu'on
        // n'a pas confondu les deux en posant la cote.
        assert!(
            wide > 2.0 * touch.x,
            "{name} : son enveloppe de {hull} texels tient dans son volume touchable, \
             donc la cote a été prise sur elle et non sur le corps"
        );
    }
}

/// La largeur de corps annoncée est celle des planches.
///
/// **Elle se remesure, elle ne se relit pas** : c'est un relevé comme les marges de
/// cadrage et la foulée, donc une planche refaite par la chaîne doit faire rougir la
/// table plutôt que de passer en silence. Sans cette épreuve, le volume touchable
/// resterait sur une mesure périmée, et le symptôme reviendrait sans sa cause.
///
/// **Le plus large des deux cycles vivants**, parce que c'est lui qui décide : une cote
/// prise sur la marche laisserait dépasser le repos de trois texels sur `d1`.
#[test]
fn le_corps_annonce_est_celui_de_la_planche() {
    let side = FRAME as u32;

    for figure in &FIGURES {
        let widest = [figure.idle, figure.walk]
            .into_iter()
            .map(|bytes| {
                let sheet = load_png_masked(bytes).expect("planche du dépôt valide");
                let (views, frames) = (sheet.height() / side, sheet.width() / side);
                (0..views)
                    .flat_map(|view| (0..frames).map(move |frame| (view, frame)))
                    .map(|(view, frame)| trunk_width(&sheet, view, frame))
                    .max()
                    .expect("une planche a des vignettes")
            })
            .max()
            .expect("deux cycles vivants");

        assert_eq!(
            figure.trunk, widest as f32,
            "{} : le corps le plus large de ses planches fait {widest} texels, et la \
             table en annonce {}",
            figure.name, figure.trunk
        );
    }
}

/// Le volume touchable ne descend jamais sous le volume de marche.
///
/// **C'est le plancher, et il sert `d3`** : son corps est dessiné sur `0,66`, soit moins
/// que les `0,70` du gabarit de marche, et suivre la mesure l'aurait rendue plus dure à
/// toucher qu'avant le correctif. Un correctif qui dégrade un cas n'en est pas un.
#[test]
fn le_volume_touchable_ne_retrecit_jamais() {
    for figure in &FIGURES {
        let touch = figure.touch();
        assert!(
            touch.x >= HALF.x && touch.y >= HALF.y && touch.z >= HALF.z,
            "{} : son volume touchable {touch:?} est plus étroit que son gabarit de \
             marche {HALF:?}",
            figure.name
        );
    }
}
