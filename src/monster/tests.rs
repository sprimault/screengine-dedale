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
//! **Aucune empreinte d'image.** La conformance est l'affaire du moteur, qui la
//! tient sur ses propres scènes ; une seconde suite d'empreintes ici le mesurerait
//! deux fois et casserait à chaque changement d'habillage.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;
use crate::test_support::{SEEDS, maze};

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

/// La créature naît hors du solide, quelle que soit la graine.
///
/// **Elle emprunte le chemin du joueur et doit donc en tirer la même garantie** :
/// la pose se résout par un balayage, et un gabarit plus étroit ne change pas ce
/// que le décor laisse de place. C'est aussi ce qui dit que l'extraction du corps
/// sert vraiment à autre chose qu'au joueur.
#[test]
fn le_demon_nait_hors_du_solide() {
    for seed in SEEDS {
        let (grid, map) = maze(seed);
        let monster = Monster::new(&grid, &map).expect("planche du dépôt valide");
        let centre = monster.at();
        let cell = monster.body.cell();
        assert_ne!(cell, 0, "graine {seed:#x} : le démon naît hors du décor");

        // Un pas nul ne rencontre rien, par contrat : la plus courte sonde qui dise
        // quelque chose est une descente.
        let below = Vec3::new(centre.x, centre.y, centre.z - 1.0);
        let hit = map
            .sweep(cell, HALF, centre, below)
            .expect("la cellule du démon existe");
        assert!(
            !hit.start_solid,
            "graine {seed:#x} : le démon naît dans le solide en {centre:?}, \
             contre la surface {} de normale {:?}",
            hit.surface, hit.normal
        );
    }
}

/// Et il naît **posé** : ce qu'il a sous les pieds l'arrête aussitôt.
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
        let monster = Monster::new(&grid, &map).expect("planche du dépôt valide");
        let centre = monster.at();

        let below = Vec3::new(centre.x, centre.y, centre.z - PROBE);
        let hit = map
            .sweep(monster.body.cell(), HALF, centre, below)
            .expect("la cellule du démon existe");
        assert!(
            hit.fraction * PROBE <= HALF.z / 1024.0,
            "graine {seed:#x} : le démon descend de {} avant de toucher, \
             depuis {centre:?}",
            hit.fraction * PROBE
        );
        assert!(
            hit.normal.z > 0.5,
            "graine {seed:#x} : ce qui le porte a pour normale {:?}",
            hit.normal
        );
    }
}

/// Le bas du dessin tombe sur les pieds du corps, au cadrage près.
///
/// **C'est la relation que rien d'autre ne tient** : le volume et le dessin sont
/// deux choses, et seule cette fonction les raccorde. Un signe inversé ou un
/// facteur deux oublié ferait flotter la créature ou l'enterrerait jusqu'aux
/// genoux — visible à l'écran, mais seulement une fois qu'on sait quoi regarder.
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

    for centre in [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(12.0, -3.5, 7.25),
        Vec3::new(-40.0, 60.0, -2.0),
    ] {
        let placed = anchor(centre);

        assert_eq!(placed.x, centre.x);
        assert_eq!(placed.y, centre.y);

        // Le bas de la vignette, puis ce que le cadrage y laisse de vide : la
        // somme des deux est la cote des pieds dessinés.
        let bottom = placed.z - SPRITE_HALF;
        let drawn = bottom + MARGIN / FRAME * (2.0 * SPRITE_HALF);

        assert!(
            (drawn - (centre.z - HALF.z)).abs() <= SLACK,
            "pour un corps centré en {centre:?}, les pieds dessinés sont en {drawn} \
             et le bas du corps en {}",
            centre.z - HALF.z
        );
    }
}

/// La marge annoncée est celle que la planche de repos laisse vraiment.
///
/// **Elle est née d'un symptôme d'écran** : la créature paraissait sauter sur
/// place, et la mesure a écarté la cause supposée — au repos, le bas de la
/// silhouette ne bouge d'aucun texel d'une vue ou d'une trame à l'autre. Ce qu'elle
/// a trouvé à la place est que la marge valait **trois** et non deux, donc que la
/// créature flottait d'un texel.
///
/// **Deux exigences, et la première est la plus précieuse** : que le vide sous les
/// pieds soit le même partout dans la planche de repos, faute de quoi la silhouette
/// sauterait vraiment ; et qu'il vaille ce que [`MARGIN`] annonce, faute de quoi
/// l'ancrage est juste mais posé au mauvais endroit.
///
/// **Elle ne porte que la planche de repos.** Une marche lève les pieds — relevé à
/// deux texels d'écart —, donc exiger d'elle un bas constant serait exiger qu'un
/// marcheur ne marche pas.
#[test]
fn la_marge_annoncee_est_celle_de_la_planche() {
    let sheet = load_png_masked(IDLE).expect("planche du dépôt valide");
    let side = FRAME as u32;
    let (views, frames) = (sheet.height() / side, sheet.width() / side);

    for view in 0..views {
        for frame in 0..frames {
            assert_eq!(
                hollow(&sheet, view, frame) as f32,
                MARGIN,
                "la vue {view}, trame {frame}, laisse {} texel(s) sous les pieds",
                hollow(&sheet, view, frame)
            );
        }
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
    let mut monster = Monster::new(&grid, &map).expect("planche du dépôt valide");
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
/// **C'est la règle que ce lot écrit alors que rien ne marche encore**, et elle a
/// sa place ici parce qu'un cycle choisi ailleurs serait à défaire : une créature
/// qui marcherait en montrant des poses de repos ne se verrait pas tout de suite,
/// et le cycle de marche est précisément indexé sur ce que le décor a laissé
/// parcourir.
///
/// **La phase repart de zéro au changement**, les deux cycles n'ayant pas la même
/// foulée : reportée, elle tomberait au milieu d'un pas.
#[test]
fn une_distance_parcourue_fait_passer_a_la_marche() {
    /// Le pas d'une image, à soixante par seconde.
    const DT: f32 = 1.0 / 60.0;

    let (grid, map) = maze(SEEDS[0]);
    let mut monster = Monster::new(&grid, &map).expect("planche du dépôt valide");

    // Quelques images de repos, de quoi charger la phase.
    for _ in 0..30 {
        monster.advance(0.0, DT);
    }
    assert!(monster.phase > 0.0, "le repos n'a pas avancé sa phase");

    monster.advance(STRIDE / 4.0, DT);
    assert_eq!(
        monster.motion,
        Motion::Walk,
        "une distance parcourue n'a pas fait passer à la marche"
    );
    assert!(
        (monster.phase - 0.25).abs() < 1.0e-6,
        "un quart de foulée met le cycle de marche à {} tour",
        monster.phase
    );

    // Et le retour au repos : la phase repart de zéro, elle ne se reporte pas.
    monster.advance(0.0, DT);
    assert_eq!(monster.motion, Motion::Idle);
    assert!(
        (monster.phase - DT / IDLE_PERIOD).abs() < 1.0e-6,
        "au retour au repos, la phase est à {} au lieu de repartir de zéro",
        monster.phase
    );
}
