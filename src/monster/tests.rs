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
        for (bytes, motion) in [(figure.idle, Motion::Idle), (figure.walk, Motion::Walk)] {
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
