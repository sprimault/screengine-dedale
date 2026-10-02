// Copyright 2026 Stéphane Primault <sprimault@users.noreply.github.com>
// SPDX-License-Identifier: MIT OR Apache-2.0

//! La grille du labyrinthe, et le creusement qui l'ouvre.
//!
//! Une case porte les six murs qui l'entourent, un bit chacun ; percer un mur
//! ouvre un passage entre deux cases, et l'export en tirera une cellule par case
//! et un portail par mur percé.
//!
//! Le creusement avance de proche en proche : depuis la case courante, tirer au
//! hasard un voisin encore vierge et percer le mur qui les sépare. Quand aucun
//! voisin ne l'est plus, **chasser** — balayer la grille en serpentin jusqu'à
//! retrouver une case déjà atteinte dont un voisin reste vierge — et repartir de
//! là. Il n'y a donc ni pile ni retour arrière, et le balayage reprend où il
//! s'était arrêté plutôt qu'au début : c'est ce qui le garde linéaire au total.
//!
//! La grille est percée en exactement `cases − 1` coups, soit les arêtes d'un arbre
//! couvrant : entre deux cases il existe une route, et une seule.

use std::collections::VecDeque;

#[cfg(test)]
mod tests;

/// Un des six côtés d'une case.
///
/// L'ordre des variantes est celui des bits qui portent leurs murs, et il porte
/// deux calculs : le bit est `1 << rang`, et le côté opposé est le rang voisin,
/// `rang ^ 1`. Les réordonner casserait les deux.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// Vers les `x` décroissants.
    West,
    /// Vers les `x` croissants.
    East,
    /// Vers les `y` décroissants.
    South,
    /// Vers les `y` croissants.
    North,
    /// Vers l'étage du dessous.
    Down,
    /// Vers l'étage du dessus.
    Up,
}

impl Side {
    /// Les six côtés, dans l'ordre des bits.
    pub const ALL: [Self; 6] = [
        Self::West,
        Self::East,
        Self::South,
        Self::North,
        Self::Down,
        Self::Up,
    ];

    /// Le pas vers la case que ce côté sépare de la courante.
    pub fn step(self) -> (i32, i32, i32) {
        match self {
            Self::West => (-1, 0, 0),
            Self::East => (1, 0, 0),
            Self::South => (0, -1, 0),
            Self::North => (0, 1, 0),
            Self::Down => (0, 0, -1),
            Self::Up => (0, 0, 1),
        }
    }

    /// Le côté par lequel la voisine voit le même mur.
    pub fn facing(self) -> Self {
        match self {
            Self::West => Self::East,
            Self::East => Self::West,
            Self::South => Self::North,
            Self::North => Self::South,
            Self::Down => Self::Up,
            Self::Up => Self::Down,
        }
    }

    /// Vrai pour les deux côtés qui changent d'étage.
    pub fn is_vertical(self) -> bool {
        matches!(self, Self::Down | Self::Up)
    }

    /// Le bit du mur de ce côté.
    fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// Les six murs intacts, qui valent aussi « jamais atteinte ».
///
/// Les deux sens dans la même valeur, et c'est ce qui réduit le test de
/// candidature d'un voisin à une seule comparaison : le creusement perce un mur
/// chaque fois qu'il entre dans une case, donc une case encore intacte est une
/// case où il n'est jamais entré. C'est aussi ce qui rend les bordures inertes
/// sans un test d'indice — elles valent zéro.
const INTACT: u8 = 0b11_1111;

/// Ce qui décide d'un labyrinthe.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Le nombre de cases en largeur, en profondeur et en étages.
    ///
    /// Aucune des trois ne peut être nulle. Une grille d'une seule case est
    /// légitime et n'a aucun mur à percer ; sa case reste donc intacte, ce qui
    /// est le seul cas où « intacte » ne veut pas dire « jamais atteinte ».
    pub extent: (u32, u32, u32),
    /// La graine : la même rend le même labyrinthe, et c'est ce qui rend
    /// reproductible tout ce qui s'appuie dessus — un monstre mal placé, un
    /// portail qui ne s'apparie pas, une divergence de lightmap.
    pub seed: u64,
    /// Une chance sur combien qu'un passage d'étage soit retenu comme candidat.
    ///
    /// Sans ce frein, un labyrinthe à plusieurs étages devient une éponge où
    /// plus rien ne se lit : la moitié des voisins d'une case sont verticaux dès
    /// le deuxième étage. Un sur huit laisse quelques montées par niveau.
    ///
    /// Vaut au moins un, et un passage vertical reste retenu quand il est le
    /// **seul** candidat — sinon des cases deviendraient inatteignables et le
    /// creusement ne finirait pas.
    pub vertical_odds: u32,
}

/// Une grille de cases percée en labyrinthe.
pub struct Grid {
    /// Le nombre de cases sur chaque axe.
    extent: (u32, u32, u32),
    /// Les murs de chaque case, bordures comprises.
    ///
    /// Une couche de bordure entoure la grille sur ses six faces, à zéro : le
    /// creusement ne retenant qu'une case intacte, elle n'est jamais candidate,
    /// et la boucle n'a donc aucun test d'indice à porter.
    walls: Vec<u8>,
    /// La case où l'on entre.
    start: (u32, u32, u32),
    /// La case par où l'on sort.
    exit: (u32, u32, u32),
}

impl Grid {
    /// Engendre un labyrinthe.
    ///
    /// # Panique
    ///
    /// Si une dimension est nulle, ou si `vertical_odds` l'est — ce sont des
    /// réglages que l'appelant écrit, pas des entrées qu'il reçoit.
    pub fn generate(settings: Settings) -> Self {
        let (width, height, levels) = settings.extent;
        assert!(
            width > 0 && height > 0 && levels > 0,
            "une grille sans case n'est pas un labyrinthe : {:?}",
            settings.extent
        );
        assert!(settings.vertical_odds > 0, "vertical_odds vaut au moins un");

        let span = (width as usize + 2) * (height as usize + 2) * (levels as usize + 2);
        let walls = vec![0u8; span];

        let mut grid = Self {
            extent: settings.extent,
            walls,
            start: (0, 0, 0),
            exit: (0, 0, 0),
        };

        for z in 0..levels as i32 {
            for y in 0..height as i32 {
                for x in 0..width as i32 {
                    grid.put(x, y, z, INTACT);
                }
            }
        }

        // Le centre plutôt qu'un coin : le creusement y a ses six voisins, et la
        // chasse n'a donc rien à faire avant plusieurs dizaines de coups.
        let origin = ((width / 2) as i32, (height / 2) as i32, (levels / 2) as i32);
        grid.dig(origin, settings);

        grid.start = (origin.0 as u32, origin.1 as u32, origin.2 as u32);
        grid.exit = grid.farthest(grid.start);
        grid
    }

    /// Le nombre de cases sur chaque axe.
    pub fn extent(&self) -> (u32, u32, u32) {
        self.extent
    }

    /// Le nombre total de cases.
    pub fn count(&self) -> u32 {
        self.extent.0 * self.extent.1 * self.extent.2
    }

    /// La case où l'on entre.
    pub fn start(&self) -> (u32, u32, u32) {
        self.start
    }

    /// La case par où l'on sort.
    ///
    /// C'est la plus éloignée du départ en nombre de passages, et non une case
    /// tirée au hasard : un tirage la pose une fois sur dix à côté de l'entrée,
    /// et un labyrinthe qu'on traverse en trois pas ne se remarque qu'une fois
    /// joué. Elle peut en revanche être proche **à vol d'oiseau** sans que ce
    /// soit un défaut : le chemin passe alors par un autre étage.
    pub fn exit(&self) -> (u32, u32, u32) {
        self.exit
    }

    /// Vrai si un mur ferme ce côté de cette case.
    ///
    /// Les murs extérieurs sont toujours présents : le creusement ne franchit
    /// jamais une bordure, et une case du bord garde donc la face qui donne sur
    /// le vide. C'est ce qui laisse chaque cellule fermée à l'export.
    pub fn has_wall(&self, cell: (u32, u32, u32), side: Side) -> bool {
        self.at(cell.0 as i32, cell.1 as i32, cell.2 as i32) & side.bit() != 0
    }

    /// La case voisine par ce côté, ou rien si elle est hors de la grille.
    pub fn neighbour(&self, cell: (u32, u32, u32), side: Side) -> Option<(u32, u32, u32)> {
        let (dx, dy, dz) = side.step();
        let (x, y, z) = (cell.0 as i32 + dx, cell.1 as i32 + dy, cell.2 as i32 + dz);
        let (width, height, levels) = self.extent;
        if x < 0 || y < 0 || z < 0 || x >= width as i32 || y >= height as i32 || z >= levels as i32
        {
            return None;
        }
        Some((x as u32, y as u32, z as u32))
    }

    /// Les murs d'une case, bordure comprise.
    ///
    /// Les coordonnées vont de `-1` à la dimension incluse : un pas de plus
    /// sortirait du tampon, et le creusement ne l'atteint pas puisqu'une bordure
    /// n'est jamais candidate.
    fn at(&self, x: i32, y: i32, z: i32) -> u8 {
        self.walls[self.offset(x, y, z)]
    }

    /// Remplace les murs d'une case.
    fn put(&mut self, x: i32, y: i32, z: i32, walls: u8) {
        let offset = self.offset(x, y, z);
        self.walls[offset] = walls;
    }

    /// Le rang d'une case dans le tampon, bordure décalée d'un.
    fn offset(&self, x: i32, y: i32, z: i32) -> usize {
        let (width, height, _) = self.extent;
        let pitch = width as usize + 2;
        let slice = pitch * (height as usize + 2);
        (z + 1) as usize * slice + (y + 1) as usize * pitch + (x + 1) as usize
    }

    /// Perce la grille entière depuis une case.
    ///
    /// Un tour de boucle perce exactement un mur, et il y en a `cases − 1` à
    /// percer : la chasse se déroule donc *dans* un tour, sans en consommer.
    fn dig(&mut self, origin: (i32, i32, i32), settings: Settings) {
        let mut rng = Rng::new(settings.seed);
        let mut hunt = Hunt::new(self.extent);
        let mut cell = origin;

        for _ in 1..self.count() {
            let mut candidates = self.candidates(cell, &mut rng, settings.vertical_odds);
            if candidates.is_empty() {
                cell = hunt.next(self, &mut rng, settings.vertical_odds);
                candidates = self.candidates(cell, &mut rng, settings.vertical_odds);
            }

            let side = candidates[rng.below(candidates.len() as u32) as usize];
            self.open(cell, side);
            let (dx, dy, dz) = side.step();
            cell = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
        }
    }

    /// Les côtés d'une case qui mènent vers un voisin encore vierge.
    ///
    /// Les passages verticaux passent par le frein de `vertical_odds`, sauf
    /// quand ils sont les seuls : une case dont la seule issue monte doit rester
    /// atteignable, faute de quoi le creusement tournerait sans fin.
    fn candidates(&self, cell: (i32, i32, i32), rng: &mut Rng, odds: u32) -> Vec<Side> {
        let mut flat = Vec::with_capacity(6);
        let mut climbing = Vec::with_capacity(2);
        for side in Side::ALL {
            let (dx, dy, dz) = side.step();
            if self.at(cell.0 + dx, cell.1 + dy, cell.2 + dz) != INTACT {
                continue;
            }
            if side.is_vertical() {
                climbing.push(side);
            } else {
                flat.push(side);
            }
        }

        if flat.is_empty() {
            return climbing;
        }
        for side in climbing {
            if rng.below(odds) == 0 {
                flat.push(side);
            }
        }
        flat
    }

    /// Perce le mur entre une case et sa voisine, des deux côtés.
    fn open(&mut self, cell: (i32, i32, i32), side: Side) {
        let near = self.at(cell.0, cell.1, cell.2) & !side.bit();
        self.put(cell.0, cell.1, cell.2, near);

        let (dx, dy, dz) = side.step();
        let (nx, ny, nz) = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
        let far = self.at(nx, ny, nz) & !side.facing().bit();
        self.put(nx, ny, nz, far);
    }

    /// La case la plus éloignée d'une autre, en nombre de passages franchis.
    ///
    /// À égalité, la première rencontrée par le parcours en largeur, donc la
    /// première dans l'ordre des côtés : le résultat reste fonction de la graine
    /// seule.
    fn farthest(&self, from: (u32, u32, u32)) -> (u32, u32, u32) {
        let mut seen = vec![false; self.walls.len()];
        let mut queue = VecDeque::new();

        seen[self.offset(from.0 as i32, from.1 as i32, from.2 as i32)] = true;
        queue.push_back(from);
        let mut last = from;

        while let Some(cell) = queue.pop_front() {
            last = cell;
            for side in Side::ALL {
                if self.has_wall(cell, side) {
                    continue;
                }
                let Some(next) = self.neighbour(cell, side) else {
                    continue;
                };
                let offset = self.offset(next.0 as i32, next.1 as i32, next.2 as i32);
                if seen[offset] {
                    continue;
                }
                seen[offset] = true;
                queue.push_back(next);
            }
        }
        last
    }
}

/// Le curseur de la chasse.
///
/// Il garde sa position entre deux chasses et change de sens au bord — un
/// serpentin qui reprend où il s'était arrêté. Repartir du début à chaque fois
/// rendrait le creusement quadratique sur une grande grille, pour le même
/// résultat.
struct Hunt {
    /// Le nombre de cases sur chaque axe.
    extent: (u32, u32, u32),
    /// La case où le balayage s'est arrêté.
    cell: (i32, i32, i32),
    /// Le sens du balayage sur l'axe des `x`.
    heading: i32,
}

impl Hunt {
    /// Un curseur posé sur la première case.
    fn new(extent: (u32, u32, u32)) -> Self {
        Self {
            extent,
            cell: (0, 0, 0),
            heading: 1,
        }
    }

    /// La prochaine case déjà atteinte qui garde un voisin vierge.
    ///
    /// Le balayage avance avant de tester, donc il ne rend jamais deux fois la
    /// même case sans avoir parcouru la grille entre-temps. Il termine parce que
    /// l'ensemble atteint et l'ensemble vierge sont tous deux non vides tant
    /// qu'il reste à percer : une arête les sépare, donc une telle case existe.
    fn next(&mut self, grid: &Grid, rng: &mut Rng, odds: u32) -> (i32, i32, i32) {
        loop {
            self.advance();
            let cell = self.cell;
            if grid.at(cell.0, cell.1, cell.2) == INTACT {
                continue;
            }
            if !grid.candidates(cell, rng, odds).is_empty() {
                return cell;
            }
        }
    }

    /// Avance d'une case, en serpentin, et reboucle sur la grille entière.
    fn advance(&mut self) {
        let (width, height, levels) = self.extent;
        self.cell.0 += self.heading;
        if self.cell.0 >= 0 && self.cell.0 < width as i32 {
            return;
        }

        self.heading = -self.heading;
        self.cell.0 += self.heading;
        self.cell.1 += 1;
        if self.cell.1 < height as i32 {
            return;
        }

        self.cell.1 = 0;
        self.cell.2 += 1;
        if self.cell.2 >= levels as i32 {
            self.cell.2 = 0;
        }
    }
}

/// Un générateur xorshift64*.
///
/// Écrit ici plutôt que pris d'une bibliothèque : une graine, une suite, rien à
/// configurer, et une dépendance de moins à justifier dans l'audit des licences.
struct Rng(u64);

impl Rng {
    /// Un générateur posé sur une graine.
    fn new(seed: u64) -> Self {
        // Une graine nulle rend une suite nulle : on la déplace plutôt que de la
        // refuser, l'appelant ayant le droit d'écrire zéro sans le savoir.
        Self(if seed == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            seed
        })
    }

    /// Le prochain mot de la suite.
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Un entier de zéro à `bound` exclu.
    ///
    /// Le reste d'un tirage sur soixante-quatre bits est biaisé, mais d'au plus
    /// `2⁻⁶¹` pour les bornes employées ici, qui ne dépassent pas six : un rejet
    /// serait de la cérémonie pour un écart qu'aucune partie ne révélerait.
    fn below(&mut self, bound: u32) -> u32 {
        (self.next() % u64::from(bound)) as u32
    }
}
