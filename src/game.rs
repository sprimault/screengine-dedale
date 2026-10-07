// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! L'état de la partie — et rien du monde.
//!
//! Tout ce qui est ici est **jeté au rechargement de la carte** : la pose, la
//! cellule suivie, et plus tard la vie, le score et les munitions. Le symptôme à
//! guetter est l'inverse — un champ de jeu rangé à côté d'un identifiant de
//! cellule dans une structure du monde.
//!
//! La partie **lit** le monde, en revanche, et c'est normal : suivre sa cellule
//! demande d'interroger la carte.

use screengine_play::{Affine3, Error, FreeCamera, KeyCode, MouseButton, Tick, Vec3, World};

use crate::mark::Marks;
use crate::maze::grid::Grid;
use crate::monster::{self, Monster};
use crate::player::Player;
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

/// Ce que la partie garde entre deux images.
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
    pub fn step(&mut self, tick: &mut Tick<'_>, map: &World) -> Option<Outcome> {
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

        // **Le tir vient après le regard et après le pas**, et l'ordre compte : lu
        // avant, il serait parti de l'orientation de l'image précédente, soit un
        // cran derrière ce que la souris vient de faire. Il part donc de la pose que
        // l'image à venir montrera.
        fired.then(|| {
            self.weapon.shoot();
            let outcome = shot::resolve(
                map,
                self.camera.position,
                self.ahead(),
                self.player.eye_cell(map),
                &self.volumes(),
            );

            // **Une marque ne se pose que si le décor a gagné**, et c'est une
            // conséquence de la géométrie et non une règle de goût : quand le rayon
            // s'arrête sur une créature, il n'y a aucune surface de décor au point de
            // contact. Ce qui montrera un coup sur un démon est son recul.
            if outcome.struck == shot::Struck::Decor {
                if let (Some(at), Some(hit)) = (outcome.shot.impact(), outcome.shot.hit) {
                    self.marks.add(at, hit.normal);
                }
            }

            outcome
        })
    }

    /// Les volumes que le tir doit tester, dans l'ordre des créatures.
    ///
    /// **Le rang est le lien, et il n'y en a pas d'autre** : le tir rend le rang du
    /// volume atteint, et c'est celui de la créature dans la même tranche. Un
    /// identifiant de créature serait un champ de plus à tenir pour une
    /// correspondance que l'ordre donne déjà, et la population ne change pas en
    /// cours d'image.
    ///
    /// **Le gabarit vient du module des créatures et non du corps** : toutes le
    /// partagent, le volume tenant à ce qui doit passer dans un couloir et non au
    /// dessin.
    fn volumes(&self) -> Vec<shot::Volume> {
        self.monsters
            .iter()
            .map(|monster| shot::Volume {
                centre: monster.at(),
                half: monster::HALF,
            })
            .collect()
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
