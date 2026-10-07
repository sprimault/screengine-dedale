// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Le rayon contre le décor : d'où il part, ce qu'il rencontre, et ce qu'on peut
//! en lire.
//!
//! **Un tir se résout en deux temps, et seul le premier est ici.** Le rayon contre
//! la géométrie de cellule est l'affaire du moteur, qui rend une fraction, une
//! normale et un point de contact ; le test contre le volume d'un monstre est
//! l'affaire du jeu, qui a placé ce volume. Le second n'a pas de portail, donc pas
//! d'adjacence : rien de ce que le balayage traverse ne s'applique à lui.
//!
//! **Il ne lit ni caméra, ni horloge, ni entrée.** Une origine, une direction, une
//! cellule : le tir est une fonction d'entrées vers des sorties, donc il s'éprouve
//! sans ouvrir de fenêtre et sans reposer une pose de caméra. C'est l'appelant qui
//! compose la direction du regard, parce que c'est lui qui tient la caméra.
//!
//! **Et il ne lit pas tout ce que le moteur lui rend.** Le point de contact n'est
//! valable que si le trajet a été examiné en entier — [`Shot::impact`] dit pourquoi,
//! et c'est la seule subtilité du module.

use screengine_play::{Hit, Surfaces, Vec3, World};

#[cfg(test)]
mod tests;

/// La portée d'un tir, en unités de monde.
///
/// **Trente-deux, soit huit cases, et la borne se justifie par ce qu'on
/// distingue** : à la résolution interne basse, un démon à cette distance fait une
/// poignée de pixels, et aucun réticule ne vise. Tirer plus loin serait tirer sur ce
/// qu'on ne voit pas.
///
/// **Elle tient aussi la troncature à distance**, et c'est son second effet : la
/// borne du balayage se consomme en **cellules** et non en distance, et le trajet qui
/// la sature est le trajet dégagé — exactement un tir le long d'un couloir. Huit
/// cases en restent très loin, même en comptant les paliers et les cages qu'un rayon
/// traverse en chemin.
pub const RANGE: f32 = 32.0;

/// Un tir, et ce que le décor lui a opposé.
///
/// **Le trajet demandé est gardé avec le résultat**, et ce n'est pas de la
/// commodité : la fraction que le moteur rend est relative à ce trajet, donc elle ne
/// veut rien dire sans lui. C'est aussi ce qui permettra à l'étape suivante de
/// comparer cette fraction à celle d'un volume de monstre — à condition que les deux
/// portent sur le même segment.
#[derive(Clone, Copy)]
pub struct Shot {
    /// D'où le rayon part : l'œil, en coordonnées de monde.
    pub from: Vec3,
    /// Où il s'arrête faute d'avoir rencontré quoi que ce soit, donc à sa portée.
    pub to: Vec3,
    /// La cellule donnée au balayage.
    pub cell: u32,
    /// Ce que le décor a opposé, ou rien si le tir est parti de nulle part.
    pub hit: Option<Hit>,
}

impl Shot {
    /// Vrai si le décor a arrêté le rayon avant sa portée.
    ///
    /// **Le critère est la fraction, jamais l'identifiant de surface.** Celui-ci
    /// vaut zéro pour trois cas distincts dont deux arrêtent : un portail non
    /// apparié, qui est un mur comme un autre, et un trajet tronqué. C'est la clause
    /// que la politique de collision tient déjà pour le balayage d'un corps, et le
    /// tir n'a aucune raison de la lire autrement.
    pub fn blocked(&self) -> bool {
        self.hit.is_some_and(|hit| hit.fraction < 1.0)
    }

    /// Où le rayon a touché le décor, quand on peut le savoir.
    ///
    /// **Rien quand le trajet a été tronqué**, et c'est tout l'intérêt de la
    /// méthode. Le moteur borne la région qu'il examine à un nombre de cellules ;
    /// au-delà, il recule la fraction, efface la surface et la normale — réponse
    /// conservatrice, puisque déclarer le trajet libre ferait traverser un mur qu'il
    /// n'a pas regardé. Mais **il laisse le point de contact tel qu'il l'avait
    /// initialisé**, c'est-à-dire au bout du trajet demandé : une marque posée là se
    /// poserait au-delà de ce qui a été examiné.
    ///
    /// La portée du tir rend ce cas inatteignable en jeu. La méthode ne s'y fie pas
    /// pour autant : c'est une coïncidence de réglages, pas une garantie, et elle
    /// cesserait de tenir le jour où la portée bougerait.
    pub fn impact(&self) -> Option<Vec3> {
        self.hit
            .filter(|hit| hit.fraction < 1.0 && !hit.incomplete)
            .map(|hit| hit.point)
    }
}

/// Un volume que le jeu a placé, et contre lequel un tir se teste lui-même.
///
/// **Le moteur ne le connaît pas, et c'est la frontière du projet** : un monstre n'a
/// pas de portail donc pas d'adjacence, et rien de ce que le balayage traverse ne
/// s'applique à lui. Décider qu'un démon est touchable est une règle de jeu.
///
/// **Un centre et des demi-étendues, et rien de ce qui le porte** : ce module ne sait
/// pas qu'il s'agit d'une créature, ce qui le rend éprouvable sans en construire une
/// et laisse un objet posé ou une caisse passer par le même chemin.
#[derive(Clone, Copy)]
pub struct Volume {
    /// Son centre, en coordonnées de monde.
    pub centre: Vec3,
    /// Ses demi-étendues, sur les trois axes.
    pub half: Vec3,
}

/// Ce qu'un tir a finalement atteint, le décor et les volumes départagés.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Struck {
    /// Rien à portée.
    Nothing,
    /// Le décor, au point que le rayon rend.
    Decor,
    /// Un volume, par son rang dans la tranche donnée.
    Volume(usize),
}

/// Jusqu'où un volume répondait, et lequel.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Reach {
    /// Son rang dans la tranche donnée au tir.
    pub rank: usize,
    /// La fraction du trajet à laquelle il est entré.
    pub at: f32,
}

/// Un tir résolu : ce que le décor a opposé, et ce qui a été atteint.
pub struct Outcome {
    /// Le rayon contre le décor, dont la marque d'impact aura besoin.
    pub shot: Shot,
    /// Ce qui a été touché.
    pub struck: Struck,
    /// Le volume le plus proche sur le trajet, qu'il ait gagné ou non.
    ///
    /// **Il est gardé même quand le décor l'emporte**, et c'est ce qui rend un tir
    /// diagnosticable : sans lui, un démon couvert par un mur et un démon qu'aucun
    /// tir ne visait donnent la même ligne de relevé, et il n'y a plus rien pour
    /// distinguer une règle qui s'applique d'une cote qui est fausse.
    pub nearest: Option<Reach>,
}

/// L'intervalle de paramètre où un segment reste entre deux plans parallèles.
///
/// **Le cas parallèle se traite avant de diviser**, parce qu'un segment dont la
/// composante est nulle sur cet axe ne franchit jamais les deux plans : il est dedans
/// pour tout le trajet, ou dehors pour tout le trajet.
///
/// **Ce n'est pas ce bras qui tient la correction, et il ne faut pas le croire.** Sans
/// lui, une composante nulle donnerait `±∞` — ce qui est exact et que l'intersection
/// absorbe —, et `0/0` donc un `NaN` dans le seul cas où le départ tombe
/// **exactement** sur un des deux plans ; or `f32::max` et `f32::min` rendent l'autre
/// opérande face à un `NaN`, si bien que le résultat resterait juste. Mesuré en
/// débranchant ce bras : aucune épreuve ne bouge.
///
/// **Il est là pour que la correction ne dépende pas de cela**, qui est un détail de
/// la bibliothèque et non une propriété de la méthode. Le jour où un `max` devient une
/// comparaison écrite à la main, ce bras est ce qui évite d'avoir à le savoir.
fn slab(from: f32, span: f32, centre: f32, half: f32) -> Option<(f32, f32)> {
    let (low, high) = (centre - half, centre + half);
    if span == 0.0 {
        return (from >= low && from <= high).then_some((f32::NEG_INFINITY, f32::INFINITY));
    }

    let (entry, exit) = ((low - from) / span, (high - from) / span);
    Some(match entry <= exit {
        true => (entry, exit),
        false => (exit, entry),
    })
}

/// La fraction du segment à laquelle il entre dans ce volume, s'il y entre.
///
/// **La méthode des tranches, et rien de plus coûteux.** Trois intervalles de
/// paramètre, leur intersection : neuf divisions au pire, aucune racine. Un cylindre
/// demanderait une résolution quadratique — donc une racine, donc la table du noyau —
/// pour ne gagner qu'une silhouette ronde vue de dessus, invisible avec des créatures
/// dont le dessin fait le double de la boîte.
///
/// **La fraction porte sur le segment donné**, comme celle que le moteur rend d'un
/// rayon : c'est à cette condition que les deux se comparent, et c'est tout l'enjeu du
/// départage.
///
/// **Un départ à l'intérieur du volume rend zéro.** Le segment y est déjà, donc il y
/// entre à l'instant où il part — ce qui est la réponse juste et non un cas limite :
/// un tir à bout portant dans une créature la touche.
fn enters(from: Vec3, to: Vec3, volume: &Volume) -> Option<f32> {
    let span = to - from;
    let x = slab(from.x, span.x, volume.centre.x, volume.half.x)?;
    let y = slab(from.y, span.y, volume.centre.y, volume.half.y)?;
    let z = slab(from.z, span.z, volume.centre.z, volume.half.z)?;

    let near = x.0.max(y.0).max(z.0).max(0.0);
    let far = x.1.min(y.1).min(z.1).min(1.0);
    (near <= far).then_some(near)
}

/// Tire un rayon depuis l'œil, dans la direction du regard.
///
/// `ahead` est attendu **unitaire** : il vient d'une orientation de caméra, qui en
/// rend toujours un. La portée s'applique telle quelle, donc une direction plus
/// courte raccourcirait le tir sans que rien ne le dise.
///
/// **Une cellule nulle ne tire pas**, et le partage se fait ici parce que le chemin
/// Rust du moteur ne le fait pas : sa méthode de rayon rend l'absence aussi bien pour
/// une cellule nulle que pour une cellule inconnue, là où sa frontière C en fait deux
/// cas distincts — déplacement libre d'un côté, erreur de l'autre. Zéro veut dire
/// « hors de tout volume », et un tir parti de là n'a aucun décor à rencontrer.
///
/// **Le filtre prend les surfaces solides seules.** Le décor engendré n'en écrit
/// aujourd'hui aucune autre, donc les deux filtres y voient la même chose ; celui-ci
/// est celui qui restera juste le jour où une grille ou une vitre apparaîtra, qu'un
/// tir doit traverser et qu'une sélection d'éditeur doit attraper.
pub fn fire(map: &World, from: Vec3, ahead: Vec3, cell: u32) -> Shot {
    let to = from + ahead * RANGE;
    Shot {
        from,
        to,
        cell,
        hit: match cell {
            0 => None,
            _ => map.pick(cell, from, to, Surfaces::Solid),
        },
    }
}

/// Résout un tir en entier : le décor, les volumes, et lequel des deux l'emporte.
///
/// **Le départage n'est pas une optimisation, c'est la règle.** Sans lui, un tir
/// traverse les murs : le test de tranches ne connaît rien du décor et déclare touché
/// un démon derrière une paroi. Les deux fractions se comparent parce qu'elles portent
/// sur le **même segment** — c'est ce que `fire` et `enters` ont en commun, et la
/// seule raison pour laquelle la comparaison veut dire quelque chose.
///
/// **À égalité, le décor gagne.** Le sens conservateur est celui du moteur lui-même,
/// qui préfère tronquer un trajet que le déclarer libre, et le cas ne se produit qu'au
/// bit près : un démon exactement dans le plan d'un mur est de toute façon un démon
/// qu'on ne devrait pas pouvoir toucher.
///
/// **Un départ dans le solide n'arrête rien**, et c'est le piège de la fonction. Le
/// moteur rend alors une fraction nulle : lue comme un obstacle, elle ferait gagner le
/// décor à distance zéro et **plus aucun volume ne serait jamais touchable**. Le tir
/// l'écarte donc comme obstacle — l'œil est tenu hors du solide par le balayage du
/// corps, mais rien dans le contrat ne le garantit, et perdre la touche serait un
/// symptôme bien plus difficile à lire qu'un tir qui part d'un mur.
///
/// **Une troncature, elle, arrête bien.** Le trajet a été borné sans être examiné en
/// entier, donc un volume au-delà n'est pas touché : c'est la réponse conservatrice,
/// la même que celle du moteur. La portée rend ce cas inatteignable en jeu.
pub fn resolve(map: &World, from: Vec3, ahead: Vec3, cell: u32, volumes: &[Volume]) -> Outcome {
    let shot = fire(map, from, ahead, cell);

    let decor = obstacle(shot.hit);
    let nearest = volumes
        .iter()
        .enumerate()
        .filter_map(|(rank, volume)| {
            enters(shot.from, shot.to, volume).map(|at| Reach { rank, at })
        })
        .min_by(|here, there| here.at.total_cmp(&there.at));

    let struck = arbitrate(nearest, decor);
    Outcome {
        shot,
        struck,
        nearest,
    }
}

/// La fraction à laquelle le décor fait obstacle, s'il en fait un.
///
/// **Un départ dans le solide n'est pas un obstacle à distance nulle.** Le moteur rend
/// alors une fraction nulle ; lue telle quelle, elle ferait gagner le décor contre tout
/// volume et **plus rien ne serait jamais touchable** — un tir qui cesse de porter, sans
/// que rien ne dise pourquoi. L'œil est tenu hors du solide par le balayage du corps,
/// mais le contrat ne le garantit pas.
///
/// **Séparée parce que le cas ne s'obtient pas d'ici**, et c'est mesuré : un rayon n'a
/// aucune dilatation, et l'espace compris dans l'épaisseur d'un mur n'appartient à
/// aucune cellule d'un décor fermé, si bien qu'aucun départ pris dans une cloison ne
/// lève ce statut. Le seul moyen d'éprouver la clause est donc de lui donner le contact
/// que le moteur ne produira pas.
///
/// Une troncature, en revanche, fait bien obstacle : le trajet a été borné sans être
/// examiné en entier, donc un volume au-delà n'est pas touché. C'est la réponse
/// conservatrice, la même que celle du moteur.
fn obstacle(hit: Option<Hit>) -> Option<f32> {
    hit.filter(|hit| hit.fraction < 1.0 && !hit.start_solid)
        .map(|hit| hit.fraction)
}

/// Lequel l'emporte, du décor et du volume le plus proche.
///
/// **Séparée pour être éprouvée à l'exactitude du bit.** La règle d'égalité ne se
/// vérifie pas sur un décor engendré — il faudrait y placer un volume dont la face
/// d'entrée tombe pile sur le plan d'un mur, ce qu'aucun flottant ne garantit. Ici
/// les deux fractions sont des entrées.
///
/// Les deux sont des fractions du **même** segment, ou rien quand ce côté-là
/// n'oppose pas d'obstacle.
fn arbitrate(nearest: Option<Reach>, decor: Option<f32>) -> Struck {
    match (nearest, decor) {
        // À égalité le décor gagne, d'où le `<=` : c'est le sens conservateur, et
        // c'est celui du moteur, qui préfère tronquer un trajet que le libérer.
        (Some(reach), Some(wall)) if wall <= reach.at => Struck::Decor,
        (Some(reach), _) => Struck::Volume(reach.rank),
        (None, Some(_)) => Struck::Decor,
        (None, None) => Struck::Nothing,
    }
}
