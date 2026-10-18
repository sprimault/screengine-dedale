// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Les épreuves de l'instrument de cadence : ses parts, ses moyennes, son pire, et
//! ce qu'il refuse de publier.
//!
//! **Ce qui s'éprouve est le cumul, jamais le chronométrage.** [`Pace`] lit l'horloge
//! pour borner son relevé, et aucune épreuve ne peut la reposer ; ce qui en découle
//! — la somme des parts, la division par les images, le seuil au-delà duquel un
//! relevé se tait — est une fonction des durées qu'on verse, et c'est là que les
//! défauts se logent.
//!
//! **Deux exceptions, et elles sont exactes** : le pas de jeu est versé et non mesuré,
//! et une image ne se compte qu'une fois quelle que soit la relecture de ses durées.
//! Les deux s'éprouvent donc sur [`Pace`] lui-même.
//!
//! Chacune a été vérifiée en la faisant échouer une fois, sur un code falsifié.

use super::*;

/// Une durée en millisecondes entières.
fn ms(count: u64) -> Duration {
    Duration::from_millis(count)
}

/// Ce qu'une image a coûté hors pas de jeu, les trois parts à la même durée.
///
/// Égales parce qu'aucune épreuve de [`Pace`] ne les distingue : ce qui s'y vérifie est
/// le rattachement des pas et le compte d'images, et leur répartition s'éprouve sur
/// [`Counts`], où elle se verse part par part.
fn timing(each: u64) -> FrameTime {
    FrameTime {
        submit: ms(each),
        fill: ms(each),
        screen: ms(each),
    }
}

/// Un cumul garni d'images, chacune donnée par ses quatre parts en millisecondes.
fn counted(images: &[(u64, u64, u64, u64)]) -> Counts {
    let mut counts = Counts::default();
    for &(game, scene, engine, screen) in images {
        counts.image(ms(game), ms(scene), ms(engine), ms(screen));
    }
    counts
}

/// Les quatre parts somment au coût de l'image.
///
/// **C'est la propriété qui rend le relevé lisible** : quatre nombres qui ne
/// retombent pas sur le total laissent croire à une part cachée, et c'est alors le
/// relevé qu'on soupçonne au lieu du code. Quatre valeurs distinctes, pour qu'une
/// part recopiée à la place d'une autre se voie.
#[test]
fn les_quatre_parts_somment_au_cout_de_l_image() {
    let tally = counted(&[(1, 2, 4, 8)])
        .tally(SPAN)
        .expect("une image versée");

    assert_eq!(tally.game, ms(1));
    assert_eq!(tally.scene, ms(2));
    assert_eq!(tally.engine, ms(4));
    assert_eq!(tally.screen, ms(8));
    assert_eq!(tally.image, ms(15));
}

/// La moyenne divise par les images.
///
/// Trois images de coûts différents, et aucune somme qui tombe juste par hasard sur
/// une des trois : une moyenne qui rendrait la dernière image, ou le cumul sans
/// division, se verrait toutes les deux.
#[test]
fn la_moyenne_divise_par_les_images() {
    let tally = counted(&[(1, 1, 1, 1), (2, 2, 2, 2), (3, 3, 3, 3)])
        .tally(SPAN)
        .expect("trois images versées");

    assert_eq!(tally.game, ms(2));
    assert_eq!(tally.image, ms(8));
}

/// Le pire garde l'à-coup que la moyenne efface.
///
/// **C'est la raison d'être du chiffre** : une image de vingt millisecondes noyée
/// dans vingt-neuf images courtes se voit en jouant et disparaît de la moyenne. Ce
/// qui est vérifié est que les deux ne se confondent pas.
///
/// **L'à-coup est au milieu du corpus et non au bout**, sans quoi un pire qui ne
/// garderait que la dernière image resterait vert.
#[test]
fn le_pire_garde_l_a_coup_que_la_moyenne_efface() {
    let mut images = vec![(0, 1, 1, 0); 30];
    images[15] = (0, 10, 10, 0);
    let tally = counted(&images).tally(SPAN).expect("trente images versées");

    assert_eq!(tally.peak, ms(20));
    assert!(
        tally.image < ms(3),
        "la moyenne devrait rester courte, et vaut {:?}",
        tally.image
    );
}

/// La cadence compte les images sur la durée du relevé, pas sur sa durée nominale.
///
/// Trente images en une demi-seconde font soixante par seconde. Une cadence qui
/// diviserait par [`SPAN`] en rendrait trente, et le relevé mentirait exactement
/// quand il est publié tard — ce que fait une image lente.
#[test]
fn la_cadence_compte_les_images_sur_la_duree() {
    let counts = counted(&vec![(0, 1, 1, 0); 30]);
    let tally = counts.tally(ms(500)).expect("trente images versées");

    assert!(
        (tally.rate - 60.0).abs() < 1e-3,
        "la cadence devrait valoir soixante, et vaut {}",
        tally.rate
    );
}

/// Un relevé sans image ne publie rien.
///
/// La première seconde d'un lancement, et toute seconde où la fenêtre n'a rien
/// rendu : une division par zéro image donnerait une moyenne infinie, qu'il faudrait
/// ensuite lire dans la barre de titre pour comprendre qu'elle ne veut rien dire.
#[test]
fn un_releve_sans_image_ne_publie_rien() {
    assert_eq!(Counts::default().tally(SPAN), None);
}

/// Un relevé interrompu ne publie rien, et le seuil est franc.
///
/// **Les deux bornes dans la même épreuve**, parce que c'est leur écart qui décide :
/// à [`STALE`] le relevé est tardif et vaut encore, au-delà il enjambe une attente de
/// la boucle et rendrait une cadence effondrée sans qu'aucune image n'ait coûté plus
/// cher.
#[test]
fn un_releve_interrompu_ne_publie_rien() {
    let counts = counted(&[(1, 1, 1, 1)]);

    assert!(counts.tally(STALE).is_some(), "à la borne, le relevé vaut");
    assert_eq!(counts.tally(STALE + ms(1)), None);
}

/// Une durée nulle ne publie rien.
///
/// Le cas ne vient pas de la boucle, qui ne publie qu'au-delà de [`SPAN`] : il vient
/// de ce que la clause doit tenir seule, une division par une durée nulle rendant une
/// cadence infinie plutôt qu'une erreur.
#[test]
fn une_duree_nulle_ne_publie_rien() {
    assert_eq!(counted(&[(1, 1, 1, 1)]).tally(Duration::ZERO), None);
}

/// Les pas de jeu se rattachent à l'image qui suit, et le cumul se vide avec elle.
///
/// **Un réveil tardif joue plusieurs pas pour une seule image**, et c'est le budget
/// d'une image que le relevé doit dire : deux pas versés avant une image comptent
/// pour elle. La deuxième image le vérifie dans l'autre sens — un cumul qui ne se
/// viderait pas lui ferait porter les pas de la première.
#[test]
fn les_pas_se_rattachent_a_l_image_qui_suit() {
    let mut pace = Pace::new();
    pace.stepped(ms(2));
    pace.stepped(ms(3));
    pace.rendered();
    pace.timed(timing(1));

    assert_eq!(pace.counts.game, ms(5));
    assert_eq!(pace.stepping, Duration::ZERO);

    pace.stepped(ms(7));
    pace.rendered();
    pace.timed(timing(1));

    assert_eq!(pace.counts.game, ms(12));
    assert_eq!(pace.counts.frames, 2);
}

/// Une image relue deux fois ne se compte qu'une.
///
/// **C'est une clause de l'étage d'accueil, pas une précaution** : il rend les durées
/// de la dernière image rendue, et plusieurs pas se jouent pour une seule image quand
/// la machine prend du retard — ils lisent alors tous la même valeur. Les verser
/// chacune gonflerait le compte d'images sans gonfler le temps, donc une cadence
/// inventée au moment précis où la machine décroche.
///
/// Un pas de jeu entre les deux lectures, pour que le second versement se verrait à la
/// part de jeu autant qu'au compte.
#[test]
fn une_image_relue_ne_se_compte_qu_une_fois() {
    let mut pace = Pace::new();
    pace.stepped(ms(2));
    pace.rendered();
    pace.timed(timing(4));

    pace.stepped(ms(3));
    pace.timed(timing(4));

    assert_eq!(pace.counts.frames, 1);
    assert_eq!(pace.counts.game, ms(2));
}

/// L'instrument dit qu'il attend, tant qu'aucun relevé n'est clos.
///
/// **Et il le dit plutôt que de rendre des zéros** : une cadence nulle dans la barre
/// de titre se lit comme un jeu qui ne rend rien, ce qui est le contraire de ce qui
/// se passe pendant la première seconde.
#[test]
fn l_instrument_attend_son_premier_releve() {
    assert_eq!(Pace::new().to_string(), "relevé en cours");
}

/// Le relevé tient en une ligne, et chaque nombre y est à sa place.
///
/// La barre de titre porte déjà les cotes du joueur : ce qui s'y ajoute se lit de
/// gauche à droite ou ne se lit pas. L'épreuve fige l'ordre et les unités, qui sont
/// la seule chose qu'un lecteur n'a pas sous les yeux.
#[test]
fn le_releve_tient_en_une_ligne() {
    let tally = Tally {
        rate: 60.0,
        image: Duration::from_micros(5200),
        peak: Duration::from_micros(11300),
        game: Duration::from_micros(800),
        scene: Duration::from_micros(1400),
        engine: Duration::from_micros(2700),
        screen: Duration::from_micros(300),
    };

    assert_eq!(
        tally.to_string(),
        "60 i/s, 5.2 ms (pire 11.3), jeu 0.8, scène 1.4, moteur 2.7, écran 0.3"
    );
}
