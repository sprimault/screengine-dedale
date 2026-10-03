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

use screengine_play::{Error, FreeCamera, KeyCode, MouseButton, Tick, World};

use crate::maze::grid::Grid;
use crate::player::Player;
use crate::probe::Aim;
use crate::scene::View;
use crate::weapon::Weapon;

/// Ce que la partie garde entre deux images.
pub struct Game {
    /// Le joueur : son corps, sa pose, sa cellule.
    player: Player,
    /// La caméra que le joueur dirige.
    ///
    /// **Elle ne porte plus la pose, seulement l'orientation et ce qu'elle veut
    /// parcourir** : le corps porte la position, la caméra est recalée sur son œil
    /// à chaque pas, et **ce qu'elle demande est filtré par le décor**.
    ///
    /// Le vol du moteur garde son altitude, donc le jeu y ajoute deux touches,
    /// sans quoi un décor à étages ne s'inspecte pas. Elles passent par le même
    /// balayage que le reste — l'altitude change avant que le déplacement ne soit
    /// mesuré —, donc on ne traverse ni sol ni plafond, et un escalier se gravit en
    /// montant tout en avançant. C'est ce que la gravité et le seuil de marche
    /// remplaceront.
    camera: FreeCamera,
    /// L'arme qu'il tient, et où elle en est de son balancement.
    weapon: Weapon,
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
            camera,
            player,
        })
    }

    /// Avance d'un pas : la caméra bouge, et sa cellule la suit.
    ///
    /// **Le suivi se fait ici et pas au rendu**, parce qu'il dépend de deux poses
    /// successives et qu'une image peut ne pas être dessinée à chaque pas.
    pub fn step(&mut self, tick: &mut Tick<'_>, map: &World) {
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
        // Le clic droit tire, et pour l'instant cela ne fait que montrer la pose :
        // le rayon contre le décor et le test contre une créature sont l'étape 4.
        if tick.input().button_pressed(MouseButton::Right) {
            self.weapon.shoot();
        }

        // **Ce que la caméra demande, mesuré plutôt que déduit des touches.**
        // Elle sera bientôt freinée par le décor, et c'est le corps qui portera
        // alors ce qui a vraiment été parcouru : prendre la différence des deux
        // poses laisse le filtrage s'interposer sans que rien d'autre ne bouge.
        let before = self.camera.position;
        self.camera.update(tick);
        self.hover(tick);
        let moved = self.camera.position - before;

        // Le corps rend ce qu'il a **réellement** parcouru, le décor l'ayant
        // filtré. Il porte la pose, donc la caméra se recale sur son œil : une
        // seule position fait foi, et ce n'est pas celle-ci.
        let travel = self.player.advance(map, moved);
        self.camera.position = self.player.eye();

        // **Le balancement suit cette distance et non les touches** : contre un
        // mur elle vaut zéro, donc l'arme s'arrête d'elle-même.
        self.weapon.advance(travel, self.camera.yaw, tick.dt());
    }

    /// Élève ou abaisse la caméra, à vitesse constante.
    ///
    /// **Le vol libre du moteur garde son altitude**, et c'est voulu chez lui : son
    /// déplacement suit le lacet seul, pour qu'avancer en regardant le plafond
    /// avance au lieu de monter. Mais un décor à étages ne s'inspecte pas à cote
    /// fixe — on voit l'escalier sans pouvoir le prendre.
    ///
    /// C'est donc une politique du jeu, écrite ici, et elle disparaîtra avec le
    /// déplacement du joueur : marcher, tomber et franchir une marche remplaceront
    /// ces deux touches par la gravité et un seuil.
    fn hover(&mut self, tick: &Tick<'_>) {
        /// Les unités par seconde de la montée.
        const LIFT: f32 = 4.0;

        let input = tick.input();
        // Deux touches franches et non un modificateur : `Maj` arrive bien comme
        // une touche ordinaire, mais on ne diagnostique pas un décor avec une
        // commande dont on doute.
        let rise = f32::from(input.down(KeyCode::Space)) - f32::from(input.down(KeyCode::KeyC));
        self.camera.position.z += rise * LIFT * tick.dt();
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
}
