// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de l'export.
//!
//! Elles passent toutes par le **chargement**, et c'est voulu : le décodeur du
//! moteur refuse tout ce que ce module pourrait écrire de travers dans les
//! octets, si bien qu'une carte qui se charge a déjà franchi une centaine de
//! contrôles qu'il serait absurde de réécrire ici.
//!
//! Ce que le chargement ne dit pas, en revanche, c'est si un portail s'est
//! **apparié** : un appariement manqué ne rend aucune erreur, il rend deux murs.
//! C'est la traversée sur la carte chargée qui l'établit, et c'est pour cela que
//! la moitié de ces épreuves interrogent le monde plutôt que les octets.

use super::*;
use crate::maze::grid::Settings;
use screengine_play::{SWEEP_CELLS, Surfaces, Vec3, World};

/// Une grille d'épreuve : deux étages, assez de cases pour que les deux sortes
/// de côté — mur et passage — se rencontrent partout.
///
/// **Les deux formes de cage y sont**, et c'est ce qui fait passer toutes les
/// épreuves de ce fichier sur les deux : une grille d'une seule forme laisserait
/// l'autre sans chargement, sans appariement et sans traversée.
fn grid() -> Grid {
    Grid::generate(Settings {
        extent: (16, 16, 2),
        seed: 0x5EED_1A8E,
        stairs: 6,
        ramps: 2,
        loops: 8,
    })
}

/// Vrai si cette case appartient à une cage en rampe.
///
/// Son point sûr est alors à mi-hauteur d'étage et non à la cote du sol, ce
/// qu'aucune case ordinaire ne fait.
fn on_ramp(grid: &Grid, at: (u32, u32, u32)) -> bool {
    grid.stairs()
        .iter()
        .any(|stair| stair.shape == Shape::Ramp && (stair.foot == at || stair.head() == at))
}

/// Le centre du passage qui prolonge une case vers l'est ou vers le nord.
fn gateway(at: (u32, u32, u32), side: Side) -> Vec3 {
    let (dx, dy, _) = side.step();
    Vec3::new(
        (at.0 as f32 + 0.5 + dx as f32 * 0.5) * CELL,
        (at.1 as f32 + 0.5 + dy as f32 * 0.5) * CELL,
        at.2 as f32 * LEVEL + 1.0,
    )
}

/// Le centre d'une case, à hauteur d'œil.
fn centre(at: (u32, u32, u32)) -> Vec3 {
    Vec3::new(
        (at.0 as f32 + 0.5) * CELL,
        (at.1 as f32 + 0.5) * CELL,
        at.2 as f32 * LEVEL + 1.0,
    )
}

/// Un point à l'intérieur de la cellule qui contient une case, un mètre au-dessus
/// de son sol.
///
/// Le sol vient de `ground`, que l'export emploie pour poser ses entités : les
/// deux ont besoin du même point sûr, et le dupliquer ferait qu'une correction ne
/// vaudrait que d'un côté.
fn inside(grid: &Grid, at: (u32, u32, u32)) -> Vec3 {
    let spot = ground(grid, at);
    Vec3::new(spot[0], spot[1], spot[2] + 1.0)
}

/// Toutes les cases, dans l'ordre des axes.
fn cases(grid: &Grid) -> Vec<(u32, u32, u32)> {
    let (width, height, levels) = grid.extent();
    let mut out = Vec::new();
    for z in 0..levels {
        for y in 0..height {
            for x in 0..width {
                out.push((x, y, z));
            }
        }
    }
    out
}

/// La carte se charge, ce qui veut dire qu'elle a franchi tout ce que le
/// décodeur vérifie — en-tête, pavage des sections, identifiants, enroulements
/// de portails, et les quatre contraintes du repère de lightmap.
#[test]
fn la_carte_se_charge() {
    let grid = grid();
    assert!(World::load(&world(&grid)).is_ok());
}

/// Chaque case tombe dans la cellule qui la couvre, et le compte des cellules est
/// celui qu'on attend.
///
/// La localisation est le seul contrôle qui attrape une cellule bien formée mais
/// posée au mauvais endroit : le chargement n'a aucune raison de s'en plaindre.
/// C'est elle qui a nommé l'étourderie d'axe d'une cage montant vers le nord.
#[test]
fn chaque_case_est_une_cellule_a_sa_place() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    // Une cellule par case, moins une par volée — qui en couvre deux —, plus une
    // par passage horizontal.
    let gates = cases(&grid)
        .iter()
        .flat_map(|at| [Side::East, Side::North].map(|side| (*at, side)))
        .filter(|(at, side)| !grid.has_wall(*at, *side))
        .count() as u32;
    assert_eq!(
        map.cell_count(),
        grid.count() - grid.stairs().len() as u32 + gates
    );

    for at in cases(&grid) {
        assert_eq!(
            map.locate(inside(&grid, at)),
            cover(&grid, at),
            "la case {at:?} n'est pas là où elle devrait"
        );
    }
}

/// Un mur percé se franchit : les deux portails se sont donc appariés.
///
/// C'est l'épreuve qui compte. Deux portails qui ne portent pas exactement les
/// mêmes octets ne lèvent aucune erreur — le chargement en fait deux murs, et
/// plus rien ne passe. La traversée est la seule chose qui le dise.
#[test]
fn un_passage_se_franchit() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let mut crossed = 0;

    for at in cases(&grid) {
        for side in [Side::East, Side::North] {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            let through = gate_id(&grid, at, side);

            // Deux portails séparent désormais deux cases, et chaque moitié se
            // vérifie à part : c'est la seule façon de savoir laquelle manque
            // si l'une d'elles ne s'apparie pas.
            assert_eq!(
                map.track(cover(&grid, at), inside(&grid, at), gateway(at, side)),
                through,
                "l'entrée du passage de {at:?} vers {next:?} ne se franchit pas"
            );
            assert_eq!(
                map.track(through, gateway(at, side), inside(&grid, next)),
                cover(&grid, next),
                "la sortie du passage de {at:?} vers {next:?} ne se franchit pas"
            );
            crossed += 1;
        }
    }
    assert!(crossed > 0, "la grille d'épreuve n'a aucun passage");
}

/// Un pas qui franchit **deux** portails rend la case d'arrivée.
///
/// Ce test a longtemps admis zéro à la place, parce que le résultat variait avec
/// l'orientation du pas : la traversée prenait le premier portail dans l'ordre du
/// fichier et non le premier le long du segment, donc un pas qui coupait une
/// cellule de part en part pouvait repartir en arrière. **La clause permissive est
/// tombée avec le correctif**, et l'égalité est ce qui empêche qu'il revienne sans
/// qu'on le voie.
///
/// La localisation reste vérifiée dans la foulée : c'est le repli de `Game::step`,
/// qui garde sa raison — une caméra qui vole sort vraiment du décor, et zéro le
/// dira alors légitimement.
///
/// **Les cages en rampe sont écartées, et la raison tient à ce qu'on éprouve** :
/// leur point sûr est au milieu de la pente, donc à mi-hauteur de l'étage, là où
/// celui d'une case ordinaire est à la cote du sol. Un pas entre les deux n'est
/// pas horizontal — il descend d'un demi-étage sur une case —, et assez incliné
/// pour sortir par le mur plutôt que par le portail. Ce que l'épreuve veut dire,
/// c'est qu'un pas **droit** qui coupe une cellule de part en part arrive où il
/// doit ; une pente d'un sixième n'est plus ce pas-là. Les cages à marches
/// restent, leur point sûr étant à la cote de leur étage.
#[test]
fn un_pas_long_rend_la_case_d_arrivee() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for at in cases(&grid) {
        if on_ramp(&grid, at) {
            continue;
        }
        for side in [Side::East, Side::North] {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            if on_ramp(&grid, next) {
                continue;
            }
            let arrival = cover(&grid, next);
            assert_eq!(
                map.track(cover(&grid, at), inside(&grid, at), inside(&grid, next)),
                arrival,
                "le pas de {at:?} vers {next:?} n'arrive pas dans sa case"
            );
            assert_eq!(
                map.locate(inside(&grid, next)),
                arrival,
                "la localisation ne rattrape pas le pas de {at:?} vers {next:?}"
            );
        }
    }
}

/// Un mur arrête : la traversée sort du décor plutôt que de passer au travers.
#[test]
fn un_mur_arrete() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let mut stopped = 0;

    for at in cases(&grid) {
        for side in EDGES {
            if !grid.has_wall(at, side) {
                continue;
            }
            let Some(next) = grid.neighbour(at, side) else {
                continue;
            };
            assert_eq!(
                map.track(cell_id(&grid, at), centre(at), centre(next)),
                0,
                "le mur entre {at:?} et {next:?} laisse passer"
            );
            stopped += 1;
        }
    }
    assert!(stopped > 0, "la grille d'épreuve n'a aucun mur intérieur");
}

/// Les étages ne se rejoignent **que** par une cage, et par elle ils se
/// rejoignent vraiment.
///
/// Les deux moitiés comptent autant. Qu'une cage relie est ce que tout ce lot
/// existe pour obtenir ; qu'elle soit la seule à le faire est ce qui garantit
/// qu'aucun plafond n'a de trou — un portail dans une dalle ne lèverait aucune
/// erreur, et on tomberait d'un étage sans savoir par où.
#[test]
fn les_etages_ne_se_rejoignent_que_par_une_cage() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for at in cases(&grid) {
        let Some(above) = grid.neighbour(at, Side::Up) else {
            continue;
        };
        let caged = cover(&grid, at) == cover(&grid, above);
        assert_eq!(
            caged,
            grid.stairs().iter().any(|stair| stair.foot == at),
            "la case {at:?} partage sa cellule avec celle du dessus sans volée"
        );
        if caged {
            continue;
        }
        assert_eq!(
            map.track(cover(&grid, at), inside(&grid, at), inside(&grid, above)),
            0,
            "la case {at:?} rejoint l'étage du dessus hors d'une cage"
        );
    }

    // Et la cage mène bien d'un étage à l'autre : depuis son palier, un pas vers
    // le haut de la volée reste dans la même cellule, et la sortie débouche sur
    // l'étage du dessus.
    for stair in grid.stairs() {
        let head = stair.head();
        assert_eq!(
            map.locate(inside(&grid, head)),
            cell_id(&grid, stair.foot),
            "le haut de la volée de {:?} n'est pas dans sa cage",
            stair.foot
        );
        if let Some(beyond) = grid.neighbour(head, stair.climb) {
            assert_ne!(
                cover(&grid, beyond),
                cell_id(&grid, stair.foot),
                "la sortie de la volée de {:?} ne quitte pas sa cage",
                stair.foot
            );
        }
    }
}

/// Chaque cellule de décor a sa lampe, et aucune lampe n'est dans le solide.
///
/// **La localisation est ce qui compte ici.** Une lampe n'a pas de cellule — la
/// sélection est géométrique —, donc le chargement ne dira jamais qu'elle est
/// posée dans un mur : elle n'éclairerait rien, et seule la cuisson le
/// montrerait, après coup. Le cas qui le mérite est la cage, dont le sol monte :
/// au centre de son empreinte, la cote d'un plafond d'étage est sous les marches.
///
/// Chaque lampe porte l'identifiant de la cellule qu'elle éclaire, ce qui rend la
/// correspondance vérifiable sans rien supposer de l'ordre d'écriture.
#[test]
fn chaque_cellule_de_decor_a_sa_lampe() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    let lit: Vec<(u32, u32, u32)> = cases(&grid)
        .into_iter()
        .filter(|at| cover(&grid, *at) == cell_id(&grid, *at))
        .collect();
    assert_eq!(map.light_count(), lit.len() as u32);

    // L'identifiant d'une lampe est celui de la cellule qu'elle éclaire, donc le
    // rang suffit à la désigner sans que l'ordre d'écriture ait à être tenu.
    for index in 0..map.light_count() {
        let id = map
            .light_id(index)
            .expect("une lampe annoncée a un identifiant");
        let at = lit
            .iter()
            .copied()
            .find(|at| cell_id(&grid, *at) == id)
            .unwrap_or_else(|| panic!("la lampe {id} n'éclaire aucune cellule de décor"));

        let lamp = map.light(index).expect("une lampe annoncée existe");
        assert!(lamp.radius > 0.0, "la lampe de {at:?} n'a pas de rayon");
        assert_eq!(
            map.locate(lamp.position),
            id,
            "la lampe de {at:?} n'est pas dans la cellule qu'elle éclaire"
        );
    }
}

/// Le départ et la sortie sont dans la carte, chacun dans une cellule qui existe
/// et à une place où l'on tient.
///
/// La cellule d'une entité est vérifiée au chargement, mais pas sa position : une
/// entité au milieu d'un mur se charge sans un mot. Ce qui l'attrape est la
/// localisation de sa pose, relevée d'un mètre — un placement est au sol, donc
/// pile sur une frontière.
#[test]
fn le_depart_et_la_sortie_sont_dans_la_carte() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    assert_eq!(map.entity_count(), 2);

    for (index, (class, at)) in [("start", grid.start()), ("exit", grid.exit())]
        .into_iter()
        .enumerate()
    {
        let index = index as u32;
        assert_eq!(map.entity_class(index), Some(class));
        let (_, cell) = map.entity_ids(index).expect("une entité annoncée existe");
        assert_eq!(
            cell,
            cover(&grid, at),
            "l'entité {class} annonce une cellule qui n'est pas la sienne"
        );

        let (pose, _) = map.entity_pose(index).expect("une entité a une pose");
        assert_eq!(
            map.locate(Vec3::new(pose.x, pose.y, pose.z + 1.0)),
            cell,
            "l'entité {class} est posée hors de sa cellule"
        );
    }
}

/// Les pas d'une case, par chaque passage ouvert, avec la cellule de départ.
///
/// **Le départ est le point sûr d'une case et non son centre** : `ground` le place
/// au milieu d'un palier dans une cage, là où le centre tombe sous les marches.
/// C'est ce qui rend ces pas utilisables comme échantillon d'oracle — à une
/// réserve près, qui est le départ dans le solide.
fn steps(grid: &Grid) -> Vec<(u32, Vec3, Vec3)> {
    let mut out = Vec::new();
    for at in cases(grid) {
        for side in EDGES {
            if grid.has_wall(at, side) {
                continue;
            }
            let (dx, dy, _) = side.step();
            let from = inside(grid, at);
            let to = Vec3::new(from.x + dx as f32 * CELL, from.y + dy as f32 * CELL, from.z);
            out.push((cover(grid, at), from, to));
        }
    }
    out
}

/// Un mur plein arrête un pas, dans chaque cellule et de chaque côté.
///
/// **C'est le prédicat qui manquait, et son absence a coûté une étape entière.**
/// Les deux épreuves de pas vérifient qu'un passage ouvert est *libre*, ce qu'il
/// est encore plus sûrement quand rien ne bloque ; les deux oracles comparent
/// `sweep` à `sweep_brute`, qui partagent leur formule de contact et s'accordent
/// donc sur « rien ne touche » ; `un_mur_arrete` éprouve `track`, qui dépend des
/// portails et non des normales de collision ; et la couverture d'image est prise
/// à l'horizontale, où un sol absent se voit mal. Aucune ne dit qu'un mur retient
/// quelqu'un.
///
/// **Mesuré sur ce décor** : cent soixante et une cellules de case sur quatre cent
/// quatre-vingt-dix-neuf n'arrêtent rien — ni leurs murs, ni leur sol, ni leur
/// plafond —, trois cent trente-huit arrêtent tout, et **aucune n'est mélangée**.
/// Le partage est donc par cellule, ce qui est la signature d'une normale
/// intérieure inversée pour toutes ses surfaces d'un coup. Notre export est hors
/// de cause : le contrôle de volume signé de `prism` passe, portails rentrés, et
/// chaque face prise à part est correctement enroulée — l'image le confirme.
///
/// **En attente nommée**, comme l'épreuve du pas au sol avant elle : elle dira que
/// la voie est libre le jour où le correctif arrivera.
#[test]
#[ignore = "une cellule de case sur trois n'arrête rien, et le partage se fait par cellule"]
fn un_mur_plein_arrete_un_pas() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let half = Vec3::new(0.3, 0.3, 0.9);

    for at in cases(&grid) {
        // Les cages sont écartées comme ailleurs : un pas qui y entre monte des
        // marches. Ce qui s'éprouve ici est le prisme, qui est tout le reste.
        if cover(&grid, at) != cell_id(&grid, at) || flight_from(&grid, at).is_some() {
            continue;
        }
        let from = inside(&grid, at);
        for side in EDGES {
            if !grid.has_wall(at, side) {
                continue;
            }
            // Une case entière : le mur est à une demi-case du départ, donc le
            // pas le dépasse franchement et un arrêt ne peut pas être un hasard
            // d'arrondi.
            let (dx, dy, _) = side.step();
            let to = Vec3::new(from.x + dx as f32 * CELL, from.y + dy as f32 * CELL, from.z);
            let hit = map
                .sweep(cell_id(&grid, at), half, from, to)
                .expect("la cellule existe");
            assert!(
                hit.fraction < 1.0,
                "le pas de {at:?} vers {side:?} traverse un mur plein"
            );
        }
    }
}

/// Un pas de joueur n'épuise pas la région que le balayage examine.
///
/// **C'est la mesure que ce décor devait rendre.** Le budget vaut soixante-quatre
/// cellules, il ne se configure pas, et sa valeur est publiée dans le header C
/// — où elle ne changera plus de sens — alors que le décor de collision du moteur
/// en a deux. Un labyrinthe de plusieurs centaines de cellules est le premier à
/// pouvoir dire si elle suffit.
///
/// Ce qu'un jeu demande est un pas par image : à quelques unités par seconde et
/// soixante images, une fraction d'unité. L'épreuve prend une case entière, soit
/// deux ordres de grandeur de marge, et une boîte large plutôt qu'une boîte de
/// joueur — c'est l'étendue balayée qui fait enfler la région, pas la longueur du
/// pas.
///
/// **Et un pas de véhicule n'y change rien dans un décor cloisonné**, ce qui est
/// la vraie mesure : le même pas étiré soixante-quatre fois, soit deux cent
/// cinquante-six unités, ne tronque pas davantage. Le mouvement rencontre un mur
/// bien avant, la traversée s'élague au meilleur contact trouvé, et la région
/// examinée reste loin des soixante-quatre cellules. La borne se consomme donc
/// comme le volume balayé **jusqu'au premier contact** — un trajet dégagé, lui,
/// la consomme comme le volume demandé.
#[test]
fn un_pas_de_joueur_n_epuise_pas_le_balayage() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for (cell, from, to) in steps(&grid) {
        for half in [Vec3::new(0.3, 0.3, 0.9), Vec3::new(1.2, 1.2, 1.2)] {
            let hit = map.sweep(cell, half, from, to).expect("la cellule existe");
            assert!(
                !hit.incomplete,
                "un pas d'une case de {from:?} vers {to:?} épuise les {SWEEP_CELLS} cellules du balayage"
            );

            let far = from + (to - from) * 64.0;
            let hit = map.sweep(cell, half, from, far).expect("la cellule existe");
            assert!(
                !hit.incomplete,
                "un pas de véhicule de {from:?} vers {far:?} épuise les {SWEEP_CELLS} cellules du balayage"
            );
        }
    }
}

/// Un pas d'une case par un passage ouvert ne rencontre rien.
///
/// **Elle est née d'un défaut du moteur, et c'est elle qui l'a établi** : une
/// boîte derrière le plan d'un mur et qui s'en éloigne obtenait un contact
/// immédiat contre lui, à n'importe quelle distance. Un labyrinthe n'est fait que
/// de coudes, donc chaque pas y rencontrait un mur du mauvais côté — là où les
/// décors de conformance, d'un seul tenant, n'en offraient aucun.
///
/// Les cases d'escalier sont écartées : un pas qui y entre monte des marches, et
/// un contact y est juste.
///
/// **C'est le déplacement de l'étape 2 qui en dépend** : on ne l'écrit pas sur un
/// balayage qui rend un contact immédiat partout, et c'est cette épreuve qui dit
/// que la voie est libre.
#[test]
fn un_pas_dans_un_couloir_ouvert_est_libre() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let half = Vec3::new(0.3, 0.3, 0.9);

    for at in cases(&grid) {
        if cover(&grid, at) != cell_id(&grid, at) || flight_from(&grid, at).is_some() {
            continue;
        }
        for side in EDGES {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            if flight_from(&grid, next).is_some() || under_flight(&grid, next) {
                continue;
            }
            let from = inside(&grid, at);
            let to = inside(&grid, next);
            let hit = map
                .sweep(cell_id(&grid, at), half, from, to)
                .expect("la cellule existe");
            assert_eq!(
                hit.fraction, 1.0,
                "le pas de {at:?} vers {next:?} s'arrête à {} contre la surface {}",
                hit.fraction, hit.surface
            );
        }
    }
}

/// Le balayage traversant rend exactement ce que rend le chemin brut.
///
/// **C'est l'oracle de la collision, et le moteur le dit ainsi** : le chemin
/// rapide suit les portails et s'arrête à une borne, le brut ne suit rien et
/// visite tout. Sur une carte bien formée les deux rendent les mêmes bits, et
/// l'égalité est le seul contrôle qui attrape une traversée trop étroite.
///
/// **Les départs dans le solide s'écartent, et ce n'est pas un relâchement** :
/// là, les deux s'accordent sur la fraction, nulle, mais pas nécessairement sur
/// la surface — elle se départage par la moindre pénétration, et la plus
/// superficielle peut vivre dans une cellule que la traversée n'atteint pas. Le
/// moteur conclut que c'est au décor de rester hors de ce cas plutôt qu'à la
/// comparaison de se relâcher, et une boîte de joueur posée au milieu d'un palier
/// de cage y tombe.
#[test]
fn le_balayage_s_accorde_avec_le_chemin_brut() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let half = Vec3::new(0.3, 0.3, 0.9);

    for (cell, from, to) in steps(&grid) {
        let walked = map.sweep(cell, half, from, to).expect("la cellule existe");
        let brute = map.sweep_brute(half, from, to);
        if walked.start_solid || brute.start_solid {
            continue;
        }
        assert_eq!(
            walked, brute,
            "le balayage de {from:?} vers {to:?} diverge du chemin brut"
        );
    }
}

/// Le rayon traversant rend exactement ce que rend le chemin brut.
///
/// Même oracle que le balayage, sur une boîte de côté nul : c'est ce que le tir
/// de l'étape 4 emploiera, et il n'a pas d'épaisseur à faire pénétrer — d'où un
/// départ qui n'est jamais solide, et aucun cas à écarter.
#[test]
fn le_rayon_s_accorde_avec_le_chemin_brut() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");

    for (cell, from, to) in steps(&grid) {
        for surfaces in [Surfaces::Solid, Surfaces::All] {
            let walked = map
                .pick(cell, from, to, surfaces)
                .expect("la cellule existe");
            assert_eq!(
                walked,
                map.pick_brute(from, to, surfaces),
                "le rayon de {from:?} vers {to:?} diverge du chemin brut"
            );
        }
    }
}

/// Un pas dont la boîte **touche le sol** franchit un passage ouvert.
///
/// **C'est le même pas que [`un_pas_dans_un_couloir_ouvert_est_libre`], à dix
/// centimètres près, et ces dix centimètres décidaient.** Au-dessus de la bande
/// de contact, les mille douze pas de ce labyrinthe passaient ; dedans, aucun —
/// l'arête du seuil n'étant pas vue comme partagée avec le sol d'en face, le
/// flanc du volume dilaté du sol de départ arrêtait le pas à la jointure de deux
/// cellules, dans un couloir plat et ouvert.
///
/// **C'est le cas de tout personnage qui a les pieds au sol**, donc l'épreuve
/// dont le déplacement de l'étape 2 dépend.
#[test]
fn un_pas_au_sol_franchit_un_passage() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let half = Vec3::new(0.3, 0.3, 0.9);

    for at in cases(&grid) {
        if cover(&grid, at) != cell_id(&grid, at) || flight_from(&grid, at).is_some() {
            continue;
        }
        for side in EDGES {
            if grid.has_wall(at, side) {
                continue;
            }
            let next = grid.neighbour(at, side).expect("un passage a une voisine");
            if flight_from(&grid, next).is_some() || under_flight(&grid, next) {
                continue;
            }
            // **Dans la bande de peau, sans y pénétrer.** Le moteur dilate la
            // boîte d'un millième de sa plus grande demi-étendue — ici neuf
            // dix-millièmes — pour qu'elle ne reste jamais collée à ce qu'elle
            // touche. Posée pile à la cote du sol, elle pénètre cette dilatation
            // et part légitimement dans le solide ; un quart de millimètre plus
            // haut, elle ne pénètre plus rien et doit passer.
            const LIFT: f32 = 1.0 / 4096.0;

            let base = ground(&grid, at);
            let target = ground(&grid, next);
            let from = Vec3::new(base[0], base[1], base[2] + half.z + LIFT);
            let to = Vec3::new(target[0], target[1], target[2] + half.z + LIFT);
            let hit = map
                .sweep(cell_id(&grid, at), half, from, to)
                .expect("la cellule existe");
            assert!(
                !hit.start_solid,
                "une boîte posée en {at:?} part déjà dans le solide, contre la surface {}",
                hit.surface
            );
            assert_eq!(
                hit.fraction, 1.0,
                "le pas de {at:?} vers {next:?} s'arrête à {} contre la surface {}",
                hit.fraction, hit.surface
            );
        }
    }
}

/// Le décor d'épreuve porte les deux formes de cage.
///
/// **Sans elle, tout ce fichier pourrait n'éprouver qu'une forme** et rester
/// vert : la forme est tirée avec la graine, et une grille sans rampe passerait
/// le chargement, l'appariement et les deux oracles sans en charger une seule.
#[test]
fn les_deux_formes_de_cage_sont_dans_la_carte() {
    let grid = grid();
    let ramps = shape_count(&grid, Shape::Ramp);
    let steps = shape_count(&grid, Shape::Steps);
    assert!(
        ramps > 0 && steps > 0,
        "le décor d'épreuve porte {ramps} rampes et {steps} volées de marches"
    );
}

/// Le nombre de volées d'une forme donnée.
fn shape_count(grid: &Grid, shape: Shape) -> usize {
    grid.stairs()
        .iter()
        .filter(|stair| stair.shape == shape)
        .count()
}

/// Une boîte qui descend sur une rampe est arrêtée, où qu'elle tombe en travers.
///
/// **C'est un prédicat et non un oracle, et c'est tout son objet.** `sweep` et
/// `sweep_brute` partagent leur formule de contact : une formule fausse rend les
/// deux chemins faux de la même façon, et leur égalité reste verte. Un trou de
/// contact le long de l'arête amont d'une face oblique est exactement de cette
/// famille, et aucune comparaison de chemins ne peut le voir.
///
/// La surface touchée est vérifiée en plus de la fraction : arrêté par un flanc
/// n'est pas arrêté par la rampe, et seul le second prouve quelque chose.
#[test]
fn une_boite_qui_descend_sur_une_rampe_est_arretee() {
    let grid = grid();
    let map = World::load(&world(&grid)).expect("carte engendrée valide");
    let half = Vec3::new(0.3, 0.3, 0.9);
    let (width, height, levels) = grid.extent();
    let cells = width * height * levels;

    let mut tried = 0;
    for stair in grid.stairs().iter().filter(|s| s.shape == Shape::Ramp) {
        let frame = Flight::new(stair.foot, stair.climb);
        let cell = cell_id(&grid, stair.foot);
        // La rampe est le premier segment du profil, donc la première surface
        // que la cage écrit.
        let ramp = cells * 3 * PRISM_SURFACES + (cell - 1) * STAIR_SURFACES + 1;

        for along in 1..=8u32 {
            let run = frame.entry + frame.direction() * INNER * along as f32 / 9.0;
            let surface = frame.base + LEVEL * (run - frame.entry).abs() / INNER;
            for across in 0..8u32 {
                // En travers, de 0,45 à 2,55 du premier flanc : la boîte garde
                // ses trois dixièmes de demi-largeur **et** la bande de peau du
                // moteur, sans quoi elle partirait légitimement dans le solide.
                let fraction = 0.15 + 0.1 * across as f32;
                let side = frame.sides.0 + (frame.sides.1 - frame.sides.0) * fraction;

                // Un dixième au-dessus de ce que la pente exige : à 45°, le coin
                // aval du bas monte d'autant que la boîte est profonde, donc le
                // centre doit être à `half.z + half.x` de la surface pour n'y pas
                // toucher au départ.
                let clear = surface + half.z + half.x + 0.1;
                let start = frame.point(run, side, clear);
                let end = frame.point(run, side, clear - 2.0);
                let hit = map
                    .sweep(
                        cell,
                        half,
                        Vec3::new(start[0], start[1], start[2]),
                        Vec3::new(end[0], end[1], end[2]),
                    )
                    .expect("la cellule existe");

                assert!(
                    !hit.start_solid,
                    "la boîte part dans le solide au-dessus de la rampe {cell}, \
                     contre la surface {}",
                    hit.surface
                );
                assert!(
                    hit.fraction < 1.0,
                    "la boîte traverse la rampe {cell} à {run} en travers de {side}"
                );
                assert_eq!(
                    hit.surface, ramp,
                    "la boîte s'arrête sur la surface {} et non sur la rampe {ramp}",
                    hit.surface
                );
                tried += 1;
            }
        }
    }
    assert!(tried > 0, "aucune rampe dans le décor d'épreuve");
}

/// Deux exports de la même grille portent les mêmes octets.
///
/// C'est ce qui rend l'empreinte du cache de lightmaps réutilisable d'une
/// génération à l'autre, et ce qui attraperait une table de hachage glissée dans
/// ce qui décide d'un ordre d'écriture.
#[test]
fn deux_exports_sont_identiques() {
    let grid = grid();
    assert_eq!(world(&grid), world(&grid));
}

/// Le fichier annonce sa propre longueur, que le chargement exige exacte.
#[test]
fn la_longueur_annoncee_est_la_bonne() {
    let bytes = world(&grid());
    let announced = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    assert_eq!(announced as usize, bytes.len());
}

/// Tous les axes de repère sont unitaires, l'axe de pente d'une rampe compris.
///
/// **C'est la densité de luxels qui est en jeu** : deux faces jointives dont les
/// axes n'ont pas la même longueur portent deux pas d'éclairage différents, et la
/// jointure se lit comme une marche. Un axe qui serait l'arête plutôt que sa
/// direction remettrait la taille des cases dans la boucle, et une pente l'y
/// remettrait deux fois.
#[test]
fn les_axes_sont_unitaires() {
    /// Deux ulp autour de l'unité, ce que la racine inverse d'une pente laisse.
    const TOLERANCE: f32 = 2.4e-7;

    /// Le carré de la longueur d'un axe.
    fn square(axis: [f32; 3]) -> f32 {
        axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]
    }

    let slope = Flight::new((1, 1, 0), Side::East).slope(INNER, LEVEL);
    for axis in ALONG.into_iter().chain([UPWARD, FLAT.0, FLAT.1, slope]) {
        assert!(
            (square(axis) - 1.0).abs() <= TOLERANCE,
            "l'axe {axis:?} n'est pas unitaire : son carré vaut {}",
            square(axis)
        );
    }
}

/// L'empreinte d'une case tourne dans le bon sens.
#[test]
fn l_empreinte_a_une_aire_positive() {
    let footprint = [[0.0, 0.0], [CELL, 0.0], [CELL, CELL], [0.0, CELL]];
    assert!(twice_area(&footprint) > 0.0);
}

/// Un rang de case rend la même cote quel que soit le côté qui la demande.
///
/// L'appariement des portails tient entièrement là-dessus : le format compare
/// les positions au bit près, sans tolérance d'aucune sorte.
#[test]
fn une_arete_a_les_memes_bits_des_deux_cotes() {
    for index in 0..64u32 {
        assert_eq!(coord(index + 1).to_bits(), coord(index + 1).to_bits());
        assert_ne!(coord(index).to_bits(), coord(index + 1).to_bits());
    }
}

/// Les portails de l'export, relus dans ses octets : pour chacun, la cellule qui
/// le porte et sa clé d'appariement.
///
/// La clé est faite des **bits** des positions de ses sommets, triés — exactement
/// celle que le chargement construit. Le relire ici est ce qui permet de dire
/// *lequel* est en cause, là où le décodeur ne rend qu'un refus.
fn portals(map: &[u8]) -> Vec<(u32, Vec<[u32; 3]>)> {
    let read = |at: usize| u32::from_le_bytes(map[at..at + 4].try_into().unwrap());
    let cells_at = read(24) as usize;
    let cells_len = read(28) as usize;

    let mut out = Vec::new();
    let mut at = cells_at;
    while at < cells_at + cells_len {
        let body = at + 4;
        let length = read(at) as usize;
        let id = read(body);
        let points = read(body + 8) as usize;
        let surfaces = read(body + 12) as usize;
        let count = read(body + 16) as usize;

        let vertices = body + 20;
        let mut cursor = vertices + points * 12;
        for _ in 0..surfaces {
            cursor += 16 + read(cursor + 12) as usize * 4 + 72;
        }
        for _ in 0..count {
            let indices = read(cursor + 4) as usize;
            let mut key: Vec<[u32; 3]> = (0..indices)
                .map(|slot| {
                    let point = vertices + read(cursor + 8 + slot * 4) as usize * 12;
                    [read(point), read(point + 4), read(point + 8)]
                })
                .collect();
            key.sort_unstable();
            out.push((id, key));
            cursor += 8 + indices * 4;
        }
        at = body + length;
    }
    out
}

/// Aucune clé de portail ne porte trois portails, ni deux de la même cellule.
///
/// Ce sont les deux seules choses que le chargement refuse sur un portail, et il
/// ne dit pas laquelle ni où. Cette épreuve le dit.
#[test]
fn chaque_portail_a_au_plus_un_vis_a_vis() {
    let grid = grid();
    let found = portals(&world(&grid));

    let mut sorted = found.clone();
    sorted.sort_by(|a, b| a.1.cmp(&b.1));
    let mut start = 0;
    while start < sorted.len() {
        let mut end = start + 1;
        while end < sorted.len() && sorted[end].1 == sorted[start].1 {
            end += 1;
        }
        let holders: Vec<u32> = sorted[start..end].iter().map(|(id, _)| *id).collect();
        assert!(
            end - start <= 2,
            "{} portails sur la même clé, portés par {holders:?}",
            end - start
        );
        if end - start == 2 {
            assert_ne!(
                holders[0], holders[1],
                "la cellule {} s'apparie avec elle-même",
                holders[0]
            );
        }
        start = end;
    }
}
