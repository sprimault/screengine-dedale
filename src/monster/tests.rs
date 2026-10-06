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
use crate::test_support::{HEIGHT, SEEDS, WIDTH, context, frame, maze};
use screengine_play::FreeCamera;
use screengine_play::screengine::BYTES_PER_PIXEL;

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
