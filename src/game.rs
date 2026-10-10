// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! L'état de la partie — et rien du monde.
//!
//! **Deux durées de vie, et il ne faut pas les confondre.** [`Game`] porte la
//! traversée d'une carte : la pose, la cellule suivie, l'arme, les marques, les
//! créatures. Tout y est **jeté au rechargement de la carte**, et le symptôme à
//! guetter est l'inverse — un champ de jeu rangé à côté d'un identifiant de cellule
//! dans une structure du monde.
//!
//! **[`Run`] porte ce qui traverse un changement de carte** : la vie, et le score qui
//! la rejoindra. Enchaîner un labyrinthe reconstruira la traversée et gardera la
//! course, là où l'édition à chaud jette les deux. Les ranger ensemble remettrait le
//! cumul au plein à chaque niveau, par construction et sans qu'une ligne le demande.
//!
//! La partie **lit** le monde, en revanche, et c'est normal : suivre sa cellule
//! demande d'interroger la carte.

use screengine_play::{Affine3, Error, FreeCamera, KeyCode, MouseButton, Tick, Vec3, World};

use crate::body;
use crate::mark::Marks;
use crate::maze::grid::Grid;
use crate::monster::{self, Monster};
use crate::player::{self, Player};
use crate::probe::Aim;
use crate::scene::View;
use crate::shot::{self, Outcome};
use crate::weapon::Weapon;

#[cfg(test)]
mod tests;

/// La vitesse de marche, en unités de monde par seconde.
///
/// **Trois mètres vingt, et c'est un réglage d'écran** : la caméra d'inspection du
/// moteur va à quatre, soit plus de quatorze kilomètres à l'heure, ce qui faisait
/// paraître la montée d'un escalier précipitée ; à deux et demi, un couloir se
/// traverse trop lentement. La cote se juge en marchant, pas en raisonnant.
///
/// **Un seul réglage pour deux sensations, et c'est ce qui l'a fixée** : l'ascension
/// d'une volée suit la vitesse horizontale, puisqu'on franchit une marche par
/// contact. Sur la pente d'un escalier, elle vaut `WALK` fois quatre tiers — donc
/// ralentir la marche calme la montée, et l'accélérer la ravive.
const WALK: f32 = 3.2;

/// La vie du joueur au départ d'une course.
///
/// **Cent, et le chiffre n'est pas une mesure de robustesse mais de lisibilité** :
/// une jauge se lit d'autant mieux que son remplissage bouge par crans visibles, et
/// cent avec une morsure à dix en donne dix. C'est la méthode qui a fixé les trois
/// coups d'une créature — le chiffre se déduit de ce qu'il faut pour que la
/// rétroaction se voie, pas d'une idée de la difficulté.
const LIFE: u32 = 100;

/// Ce qu'une morsure coûte.
const BITE: u32 = 10;

/// Le temps entre deux morsures, en secondes.
///
/// **Dix secondes au contact d'une créature pour mourir**, avec les deux cotes
/// ci-dessus, ce qui laisse le temps de comprendre d'où le coup vient et de
/// reculer. Une morsure par image tuerait avant que la jauge ait bougé à l'écran.
///
/// **En secondes et jamais en images** : c'est la forme du recul d'une créature
/// touchée, et la raison est la même — à trente images par seconde comme à cent
/// vingt, une morsure coûte autant. Le défaut inverse existe dans ce dépôt, sur
/// l'inertie du lacet de l'arme, et il vaut un facteur vingt-trois entre les deux
/// cadences.
const RESPITE: f32 = 1.0;

/// Ce qu'un couple de touches opposées demande : `+1`, `−1`, ou rien.
///
/// **Tenues ensemble, elles s'annulent**, ce qui est la seule réponse qui ne
/// surprenne pas : un joueur qui appuie des deux côtés ne va nulle part.
fn axis(plus: bool, minus: bool) -> f32 {
    f32::from(plus) - f32::from(minus)
}

/// La direction d'un pas, pour ce lacet et ces deux commandes.
///
/// **Le déplacement suit le lacet seul, jamais le regard** : dans un couloir,
/// avancer en regardant le plafond doit avancer et non monter. C'est la clause que
/// la caméra libre du moteur tient aussi, et elle vaut encore plus ici — le tangage
/// n'a plus de déplacement vertical à donner depuis que la pesanteur est là.
///
/// **La diagonale ne va pas plus vite**, et c'est un choix : composée sans
/// normaliser, elle avancerait d'un facteur `√2`, ce qui se joue en marchant de
/// biais en permanence. Les jeux de l'époque l'ont laissé passer, et leurs joueurs
/// en ont fait une technique.
///
/// Le repère est celui du projet : la caméra neutre regarde le `+X`, son axe droit
/// est le `−Y`.
fn walk(yaw: f32, ahead: f32, side: f32) -> Vec3 {
    let (sin, cos) = yaw.sin_cos();
    let forward = Vec3::new(cos, sin, 0.0);
    let right = Vec3::new(sin, -cos, 0.0);

    let step = forward * ahead + right * side;

    // **La racine vient du noyau, et seulement quand elle sert** : le carré se
    // compare à un sans racine du tout, et `normalize` passe par la table de racine
    // inverse du moteur plutôt que par un calcul local. Ce n'est pas une
    // normalisation mais un plafond — un pas simple vaut déjà un et ne se touche
    // pas.
    match step.dot(step) > 1.0 {
        true => step.normalize(),
        false => step,
    }
}

/// Ce qu'une course garde d'un labyrinthe au suivant.
///
/// **Elle n'est pas dans [`Game`], et la raison se mesure** : `Game::new` est son
/// seul constructeur et la traversée ne détient aucune référence au monde, donc
/// enchaîner un labyrinthe la reconstruira entière et tout ce qu'elle porte repartira
/// au plein. Une vie et un score rangés là seraient refaits à chaque niveau sans
/// qu'une ligne le demande, ce qui est l'inverse de ce qu'un écran de fin cumulatif
/// veut.
///
/// **Elle se passe au pas comme la carte se passe**, plutôt que d'être tenue par la
/// traversée : ce qui n'appartient pas à `Game` lui est donné, et le compilateur rend
/// alors cette frontière aussi visible que celle du monde.
pub struct Run {
    /// Ce qui reste de vie, sur [`LIFE`].
    life: u32,
    /// Ce que les démons abattus ont valu.
    ///
    /// **C'est le champ pour lequel la course existe** : la vie se refait au plein à
    /// chaque labyrinthe sans que cela surprenne, mais un score remis à zéro en
    /// franchissant une sortie retirerait tout sens à l'écran de fin.
    score: u32,
}

impl Default for Run {
    fn default() -> Self {
        Self::new()
    }
}

impl Run {
    /// Une course qui commence, la vie au plein et le compteur à zéro.
    pub fn new() -> Self {
        Self {
            life: LIFE,
            score: 0,
        }
    }

    /// La part de vie qui reste, de zéro à un.
    ///
    /// **La jauge veut une part et non deux entiers** : ses cotes se prennent du
    /// tampon à chaque image, donc la largeur du remplissage se calcule là-bas, et
    /// lui passer le total obligerait l'interface à connaître le barème.
    pub fn share(&self) -> f32 {
        self.life as f32 / LIFE as f32
    }

    /// Vrai si le joueur est mort, donc si la course est finie.
    pub fn over(&self) -> bool {
        self.life == 0
    }

    /// Le compteur, tel que l'écran l'écrit.
    ///
    /// **Un entier et non une part, à l'inverse de la vie** : un score n'a pas de
    /// total contre lequel se rapporter, et c'est ce qui décide — la jauge montre une
    /// proportion, le compteur montre le nombre lui-même.
    pub fn score(&self) -> u32 {
        self.score
    }

    /// Porte au compteur la prime d'un démon abattu.
    ///
    /// **La prime vient de la silhouette et non d'ici**, et c'est ce qui décide de la
    /// signature : les trois ne mourront pas de la même façon, donc le barème vit
    /// dans leur table. La course ne fait que cumuler.
    ///
    /// **Elle ne garde pas le cas de la course finie**, et c'est voulu : la mort gèle
    /// la traversée avant tout tir, donc aucun coup ne peut porter après. Un garde ici
    /// serait une vérification autour de ce qui ne peut pas arriver.
    fn credit(&mut self, bounty: u32) {
        self.score += bounty;
    }

    /// Retire ce qu'une morsure coûte.
    ///
    /// **La saturation plutôt que la soustraction, et elle ne sert pas aujourd'hui** :
    /// la morsure divise la vie, donc la dixième l'amène exactement à zéro. Ce qu'elle
    /// garde est le réglage suivant — un barème qui ne diviserait plus ferait passer
    /// un entier non signé sous zéro, donc déborder par le haut, donc rendre une vie
    /// pleine à l'instant de mourir. C'est le genre de défaut qui ne se voit qu'une
    /// fois les cotes réglées à l'écran.
    fn hurt(&mut self) {
        self.life = self.life.saturating_sub(BITE);
    }
}

/// Ce que la traversée d'une carte garde entre deux images.
pub struct Game {
    /// Le joueur : son corps, sa pose, sa cellule.
    player: Player,
    /// La caméra que le joueur dirige.
    ///
    /// Le regard : son lacet, son tangage, et l'orientation qu'elle en compose.
    ///
    /// **Elle ne décide plus du déplacement, et ne porte plus la pose** : le corps
    /// porte la position et la caméra est recalée sur son œil à chaque pas. Ce qu'elle
    /// garde est ce qu'elle sait faire — composer lacet et tangage dans le bon ordre,
    /// ce qu'une reconstruction à la main pencherait de travers.
    ///
    /// **C'est sa moitié « regard » qui est appelée, pas sa mise à jour entière**, et
    /// c'est le prix du pas de côté : `update` mappe les touches latérales sur la
    /// rotation, et on ne défait pas après coup une rotation déjà appliquée. Le jeu
    /// appelle donc `look` seul et garde sa politique de marche, ce qui est sa place —
    /// le moteur ne connaît pas les règles du déplacement.
    camera: FreeCamera,
    /// L'arme qu'il tient, et où elle en est de son balancement.
    weapon: Weapon,
    /// Les marques d'impact posées sur le décor, et leur disque.
    ///
    /// **Un état de partie, donc jeté au rechargement de la carte** : les identifiants
    /// de surface y survivraient, mais un décor remplacé n'a pas à garder les impacts
    /// du précédent.
    marks: Marks,
    /// Les créatures qui parcourent le labyrinthe.
    ///
    /// **Une liste et non trois champs**, parce que rien ici ne distingue une
    /// silhouette d'une autre : ce qui les sépare vit dans leur table, et la partie
    /// n'a qu'à les faire marcher et à les soumettre. Une poursuite viendra plus tard
    /// sans que ce champ bouge.
    monsters: Vec<Monster>,
    /// Le temps avant qu'une morsure puisse à nouveau porter, en secondes.
    ///
    /// **Un seul décompte sert deux choses** : il espace les morsures, et il est
    /// l'invulnérabilité qui suit la précédente. Deux mécanismes diraient la même
    /// chose, et le second finirait par ne plus s'accorder au premier.
    ///
    /// **Il est de la traversée et non de la course**, donc jeté avec la carte : une
    /// morsure commencée n'a rien à reporter au labyrinthe suivant.
    respite: f32,
}

/// Ce qu'un pas fait du répit et de la course, au contact ou non.
///
/// Rend le répit qui reste, et retire une morsure à la course quand elle porte.
///
/// **Hors du pas de la partie parce qu'un `Tick` ne se fabrique pas** : il vient de
/// la boucle du moteur, donc une règle qui ne vivrait que dans [`Game::step`] ne
/// s'éprouverait qu'à l'écran. Ce qui est une fonction d'entrées vers des sorties en
/// sort, et la cadence en fait partie — c'est le seul endroit où le pas de temps
/// multiplie quelque chose, donc le seul où l'oublier coûterait.
fn bite(respite: f32, dt: f32, touching: bool, run: &mut Run) -> f32 {
    let left = (respite - dt).max(0.0);
    match left == 0.0 && touching {
        true => {
            run.hurt();
            RESPITE
        }
        false => left,
    }
}

impl Game {
    /// Une partie qui commence à l'entrée du labyrinthe, le joueur debout.
    ///
    /// # Erreurs
    ///
    /// Si la planche de l'arme ne se décode pas — elle est intégrée au binaire,
    /// donc jamais en pratique, mais un `expect` sur un chemin atteignable n'a pas
    /// sa place et l'appelant sait déjà rendre compte d'une erreur.
    pub fn new(grid: &Grid, map: &World) -> Result<Self, Error> {
        let player = Player::spawn(grid, map);
        let camera = FreeCamera::new(player.eye());
        Ok(Self {
            weapon: Weapon::new(camera.yaw)?,
            monsters: monster::population(grid, map)?,
            marks: Marks::new(),
            respite: 0.0,
            camera,
            player,
        })
    }

    /// Avance d'un pas : la caméra bouge, sa cellule la suit, et rend le tir du pas.
    ///
    /// **Le suivi se fait ici et pas au rendu**, parce qu'il dépend de deux poses
    /// successives et qu'une image peut ne pas être dessinée à chaque pas.
    ///
    /// **Le tir est rendu plutôt que gardé**, et c'est ce qui le distingue du reste :
    /// un tir est un événement d'un seul pas, là où la pose et la cellule sont des
    /// états. Le garder en champ obligerait à l'effacer, donc à décider quand — et
    /// le relevé, qui est son seul lecteur aujourd'hui, vit dans la boucle.
    pub fn step(&mut self, tick: &mut Tick<'_>, map: &World, run: &mut Run) -> Option<Outcome> {
        // La vue ne suit la souris que le curseur pris, et il ne l'est pas à
        // l'ouverture : une fenêtre qui s'en emparerait laisserait chercher
        // comment le récupérer. Le clic gauche le prend, celui du milieu le rend
        // — la convention des exemples du moteur, et Échap ferme sans détour.
        if tick.input().button_pressed(MouseButton::Left) {
            tick.capture_cursor(true);
        }
        if tick.input().button_pressed(MouseButton::Middle) {
            tick.capture_cursor(false);
        }

        // **La mort gèle la traversée, et c'est tout ce que cette étape en fait** :
        // l'écran de fin et l'enchaînement d'un labyrinthe sont d'une étape qui les
        // porte, et bâcler ici un écran qu'elle refera serait du travail à défaire.
        // Rien n'avance donc — ni le regard, ni le pas, ni les créatures, ni l'arme.
        //
        // Le curseur reste réglable au-dessus, et c'est voulu : devant une image
        // figée, on veut pouvoir rendre la souris. La relance est une course neuve,
        // donc elle se construit là où les deux états vivent côte à côte.
        if run.over() {
            return None;
        }

        let fired = tick.input().button_pressed(MouseButton::Right);

        self.camera.look(tick);
        let moved = self.wanted(tick) * (WALK * tick.dt());

        // Le corps rend ce qu'il a **réellement** parcouru, le décor l'ayant
        // filtré. Il porte la pose, donc la caméra se recale sur son œil : une
        // seule position fait foi, et ce n'est pas celle-ci.
        let travel = self.player.advance(map, moved, tick.dt());
        self.camera.position = self.player.eye();

        // **Le balancement suit cette distance et non les touches** : contre un
        // mur elle vaut zéro, donc l'arme s'arrête d'elle-même.
        self.weapon.advance(travel, self.camera.yaw, tick.dt());

        // Les créatures marchent pour leur compte : elles ne poursuivent personne, la
        // navigation d'une cellule à l'autre demandant un graphe que la carte ne
        // donne pas. Chacune avance droit et fait demi-tour sur ce qui l'arrête —
        // une paroi, ou une autre créature.
        monster::stroll(&mut self.monsters, map, tick.dt());

        // **Une créature dont la chute est jouée quitte la partie**, et il n'en reste
        // rien : son ombre s'est éteinte au coup fatal, son sprite part avec elle. Le
        // retrait se fait ici plutôt que dans la promenade, qui travaille sur une
        // tranche et ne peut pas raccourcir la population.
        self.monsters.retain(|monster| !monster.spent());

        // **Le contact qui blesse est au jeu, et il ne peut pas être ailleurs** : une
        // créature n'a pas de portail donc pas d'adjacence, et rien de ce que le
        // balayage du moteur traverse ne s'applique à elle. Ce qui décide est le
        // recouvrement des deux volumes que le jeu a posés.
        //
        // **Au contact maintenu, et non à l'entrée en contact** : le régime réel est
        // celui où l'on reste collé à une créature qui ne poursuit personne, et
        // compter à l'entrée donnerait un jeu où s'y tenir ne coûte rien. Le décompte
        // court même hors contact, pour qu'un aller-retour ne morde pas deux fois.
        let touching = self.touched();
        self.respite = bite(self.respite, tick.dt(), touching, run);

        // Les éclats s'éteignent d'eux-mêmes, et il faut donc leur donner le temps qui
        // passe — les marques, elles, n'en ont pas besoin : seul l'anneau les chasse.
        self.marks.advance(tick.dt());

        // **Le tir vient après le regard et après le pas**, et l'ordre compte : lu
        // avant, il serait parti de l'orientation de l'image précédente, soit un
        // cran derrière ce que la souris vient de faire. Il part donc de la pose que
        // l'image à venir montrera.
        fired.then(|| {
            self.weapon.shoot();
            let ahead = self.ahead();
            let (volumes, targets) = self.volumes();
            let outcome = shot::resolve(
                map,
                self.camera.position,
                ahead,
                self.player.eye_cell(map),
                &volumes,
            );

            // **Une marque ne se pose que si le décor a gagné**, et c'est une
            // conséquence de la géométrie et non une règle de goût : quand le rayon
            // s'arrête sur une créature, il n'y a aucune surface de décor au point de
            // contact. Ce qui montrera un coup sur un démon est son recul.
            match outcome.struck {
                shot::Struck::Decor => {
                    if let (Some(at), Some(hit)) = (outcome.shot.impact(), outcome.shot.hit) {
                        self.marks.add(at, hit.normal);
                    }
                }
                // **Un recul et un éclat, et les deux se compensent** : le premier dit
                // que le coup a porté, le second où. Aucune marque en revanche — il
                // n'existe aucune surface de décor au point de contact, donc rien à
                // plaquer, et c'est le sprite qui s'en charge.
                shot::Struck::Volume(rank) => {
                    // **Le compte se fait là où la créature tombe**, et le coup rend ce
                    // qu'il a fait : c'est le seul instant où sa vie atteint zéro, donc
                    // le seul où un démon peut être payé une fois et une seule. La
                    // prime vient avec, parce qu'elle est de la silhouette.
                    if let Some(bounty) = self.monsters[targets[rank]].knock(ahead) {
                        run.credit(bounty);
                    }
                    if let Some(reach) = outcome.nearest {
                        let span = outcome.shot.to - outcome.shot.from;
                        self.marks.flash(outcome.shot.from + span * reach.at);
                    }
                }
                shot::Struck::Nothing => {}
            }

            outcome
        })
    }

    /// Vrai si une créature debout recouvre le volume du joueur.
    ///
    /// **Les tombées ne touchent plus**, comme elles ne sont plus touchables : une
    /// créature dont la chute se joue reste visible une seconde et deux dixièmes, et
    /// mordre pendant ce temps ferait payer un coup qui a porté.
    ///
    /// **Les deux gabarits diffèrent**, et c'est ce qui a sorti l'inégalité du module
    /// des créatures : une boîte de joueur est plus étroite que celle d'un démon, donc
    /// un test écrit sur le double d'une seule demi-étendue serait faux des deux côtés.
    ///
    /// **Et c'est le volume de marche qu'on prend ici, pas celui du tir** : mordre
    /// demande d'être contre la créature, ce qui est une question de corps ; toucher
    /// demande de viser ce qu'on voit, ce qui est une question de dessin. Le second est
    /// plus large que le premier sur deux des trois silhouettes.
    fn touched(&self) -> bool {
        self.monsters
            .iter()
            .filter(|monster| !monster.fallen())
            .any(|monster| {
                body::overlaps(
                    self.player.centre(),
                    player::HALF,
                    monster.at(),
                    monster::HALF,
                )
            })
    }

    /// Les volumes que le tir doit tester, et à quelle créature chacun appartient.
    ///
    /// **Les tombées n'en ont plus**, et c'est ainsi qu'elles cessent d'être
    /// touchables : le tir ne connaît pas la vie d'une créature, et lui apprendre
    /// serait lui faire franchir une frontière pour une règle qui se tient ici.
    ///
    /// **D'où la seconde tranche** : le tir rend un rang dans celle qu'on lui donne,
    /// qui n'est plus celui de la créature dès qu'une manque. La correspondance se
    /// porte à côté plutôt que par un identifiant dans `Monster`, qui serait un champ
    /// de plus à tenir pour une durée d'une image.
    ///
    /// **Le gabarit vient de la silhouette, et c'est ce qui le sépare du corps** : le
    /// volume de marche tient à ce qui doit passer dans un couloir, donc il est le même
    /// pour les trois ; celui-ci doit couvrir ce qu'on voit, parce qu'on vise ce qu'on
    /// voit. Les confondre faisait rater les flancs de la silhouette la plus large.
    fn volumes(&self) -> (Vec<shot::Volume>, Vec<usize>) {
        let alive = self
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.fallen());

        alive
            .map(|(rank, monster)| {
                (
                    shot::Volume {
                        centre: monster.at(),
                        half: monster.hittable(),
                    },
                    rank,
                )
            })
            .unzip()
    }

    /// La direction du regard, unitaire, en coordonnées de monde.
    ///
    /// **Le tangage en fait partie, là où la marche l'ignore** : on tire où l'on
    /// regarde, et `walk` ne prend que le lacet parce qu'avancer en regardant le
    /// plafond doit avancer et non monter.
    ///
    /// Le repère du projet : la caméra neutre regarde le `+X`. La même composition
    /// vit dans la soumission de l'arme, qui a besoin en plus de sa droite et de son
    /// haut ; à une troisième occurrence elle sera à extraire, et ce qui l'imposera
    /// est qu'elles doivent s'accorder — l'arme est posée le long de la direction où
    /// le rayon part.
    fn ahead(&self) -> Vec3 {
        Affine3::from_rotation_translation(self.camera.camera().orientation, Vec3::ZERO)
            .transform_vector(Vec3::new(1.0, 0.0, 0.0))
    }

    /// Le pas que les touches demandent, en direction seule.
    ///
    /// **`Z` `S` et les flèches haut et bas avancent, `A` `E` font le pas de côté** —
    /// les deux touches qui encadrent `Z`, là où `Q` et `D` gardent le pivot qu'elles
    /// ont toujours eu.
    ///
    /// **Le pas de côté n'a pas encore d'emploi, et c'est assumé** : dans un
    /// labyrinthe vide on explore, et tourner suffit. Il servira quand une créature
    /// avancera — esquiver en la gardant dans la vue —, et il ne coûte d'ici là que
    /// deux touches qui ne servaient à rien.
    ///
    /// Ce sont des **positions physiques** et non des lettres : les mêmes codes valent
    /// `W` `S` et `Q` `E` sur un clavier qwerty, et rien n'est à détecter.
    fn wanted(&self, tick: &Tick<'_>) -> Vec3 {
        let input = tick.input();
        let ahead = axis(input.down(KeyCode::KeyW), input.down(KeyCode::KeyS))
            + axis(input.down(KeyCode::ArrowUp), input.down(KeyCode::ArrowDown));
        let side = axis(input.down(KeyCode::KeyE), input.down(KeyCode::KeyQ));

        walk(self.camera.yaw, ahead.clamp(-1.0, 1.0), side)
    }

    /// Ce que l'image doit regarder.
    pub fn view(&self) -> View {
        View {
            camera: self.camera.camera(),
            cell: self.player.cell(),
        }
    }

    /// La cellule courante, que le plan de contrôle marque.
    pub fn cell(&self) -> u32 {
        self.player.cell()
    }

    /// Ce que le relevé note, et qu'une épreuve peut reposer.
    ///
    /// **Les deux angles plutôt que la caméra rendue** : c'est la seule forme qui
    /// se rejoue sans redeviner l'ordre de composition, et c'est de la partie qu'ils
    /// viennent — le monde n'en sait rien.
    pub fn aim(&self) -> Aim {
        Aim::new(&self.camera, self.player.cell())
    }

    /// L'arme en main, que le rendu soumet après le décor.
    pub fn weapon(&self) -> &Weapon {
        &self.weapon
    }

    /// Les créatures, que le rendu soumet entre le décor et l'arme.
    pub fn monsters(&self) -> &[Monster] {
        &self.monsters
    }

    /// Les marques d'impact, que le rendu soumet juste après le décor.
    pub fn marks(&self) -> &Marks {
        &self.marks
    }

    /// La créature la plus proche de l'œil : sa silhouette et sa distance.
    ///
    /// **La cote qu'on va régler s'affiche avant d'être réglée** : la demi-étendue
    /// d'un sprite se juge à une distance donnée, et chercher cette distance en
    /// comptant les dalles est ce qui a coûté un essai par relance quand la hauteur
    /// d'œil s'est posée.
    ///
    /// **Le nom va avec la distance depuis qu'elles sont trois** : les cotes de
    /// cadrage diffèrent d'une silhouette à l'autre, donc dire laquelle flotte
    /// demande de pouvoir la nommer. Sans population, le titre rend `aucune`.
    pub fn nearest(&self) -> (&'static str, f32) {
        self.monsters
            .iter()
            .map(|monster| {
                let gap = monster.at() - self.camera.position;
                (monster.name(), gap.dot(gap).sqrt())
            })
            .min_by(|(_, here), (_, there)| here.total_cmp(there))
            .unwrap_or(("aucune", 0.0))
    }
}
