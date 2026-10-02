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

use screengine_play::{FreeCamera, KeyCode, MouseButton, Tick, Vec3, World};

use crate::scene::View;

/// Ce que la partie garde entre deux images.
pub struct Game {
    /// La caméra que le joueur dirige.
    ///
    /// En vol libre le temps de cette étape : rien ne l'arrête, et elle traverse
    /// les murs. Le vol du moteur garde son altitude, donc le jeu y ajoute deux
    /// touches — sans quoi un décor à étages ne s'inspecte pas. Le déplacement
    /// filtré par le balayage est l'étape 2, et c'est elle qui rendra la
    /// localisation rare.
    camera: FreeCamera,
    /// La cellule qui la contient, ou zéro si elle est hors du décor.
    cell: u32,
    /// Où elle était au pas précédent.
    previous: Vec3,
}

impl Game {
    /// Une partie qui commence à l'entrée du labyrinthe.
    pub fn new(entrance: Vec3, map: &World) -> Self {
        Self {
            camera: FreeCamera::new(entrance),
            cell: map.locate(entrance),
            previous: entrance,
        }
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

        self.camera.update(tick);
        self.hover(tick);
        let position = self.camera.camera().position;

        // Le suivi transporte la cellule quand le segment franchit un portail
        // apparié, et rend zéro quand il sort par un mur. Le moteur ne se
        // relocalise jamais de lui-même : c'est à l'hôte de le demander, et une
        // caméra qui vole sort souvent. La localisation est en nombre de
        // surfaces, donc on ne la paie que là.
        let found = map.track(self.cell, self.previous, position);
        self.cell = if found == 0 {
            map.locate(position)
        } else {
            found
        };
        self.previous = position;
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
            cell: self.cell,
        }
    }

    /// La cellule courante, que le plan de contrôle marque.
    pub fn cell(&self) -> u32 {
        self.cell
    }
}
