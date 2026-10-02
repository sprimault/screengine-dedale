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
//! Le creusement perce exactement `cases − 1` murs, soit les arêtes d'un arbre
//! couvrant. Ce qui s'y ajoute ensuite est compté : chaque volée au-delà du
//! minimum et chaque raccourci ferme **une** boucle, si bien que le nombre
//! cyclomatique vaut ce qu'on a demandé et rien d'autre.
//!
//! **Les escaliers se réservent avant le creusement**, et c'est la seule voie qui
//! termine. Une cage n'a que deux ouvertures — l'entrée en bas, la sortie en
//! haut —, donc chacune de ses deux cases garde une seule issue horizontale ;
//! partout ailleurs le passage déboucherait au milieu des marches ou au-dessus du
//! vide. L'imposer après coup déconnecterait, puisque dans un arbre toute arête
//! est l'unique route vers quelque chose ; l'imposer pendant romprait l'invariant
//! qui garantit que la chasse trouve toujours. Retirer ces murs du graphe
//! **avant** rend la contrainte vraie par construction, sans un seul rejet.
//!
//! Le prix en est un contrôle : en retirant tant d'arêtes, un placement de volées
//! peut couper le graphe que le creusement emprunte, et le tirage se refait
//! jusqu'à ce qu'un parcours le déclare d'un seul tenant.

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
    /// Le nombre de volées d'escalier.
    ///
    /// Il en faut **au moins une par paire d'étages consécutifs** pour que le
    /// labyrinthe soit d'un seul tenant, et c'est le minimum que la génération
    /// impose quoi qu'on demande ici. Chacune au-delà ouvre une boucle
    /// verticale : on monte par l'une, on redescend par l'autre.
    ///
    /// **C'est le seul moyen d'avoir plusieurs cages sans découper un étage.**
    /// Dans un arbre, le nombre de passages verticaux vaut `Σ morceaux d'étage
    /// − 1` : trente-et-une volées y imposeraient trente-deux morceaux, dont la
    /// plupart ne s'atteindraient qu'en changeant de niveau. Un immeuble a
    /// plusieurs escaliers **et** des étages qu'on parcourt ; les deux ensemble
    /// demandent des boucles.
    pub stairs: u32,

    /// Le nombre de passages horizontaux en plus de ceux de l'arbre.
    ///
    /// À zéro et avec le minimum de volées, le labyrinthe est **parfait** : une
    /// route et une seule entre deux cases. Chaque unité ouvre un passage de
    /// plus, donc une boucle de plus, et le compte est exact parce qu'il se
    /// règle par construction — le nombre cyclomatique vaut `passages − cases +
    /// 1`, qu'on obtient en perçant autant de murs après l'arbre.
    pub loops: u32,
}

/// Une case qui porte une volée d'escalier.
///
/// La volée monte vers l'étage du dessus en suivant un axe horizontal : on entre
/// par la face opposée à `climb`, on sort par celle de `climb`, un étage plus
/// haut.
///
/// **Ce sont là ses deux seules ouvertures**, et non deux par case : les trois
/// autres faces de chaque case sont aveugles, et c'est ce que la réservation
/// protège. En bas, du côté de la montée, le sol est déjà arrivé à la cote de
/// l'étage suivant ; en haut, du côté de l'entrée, on surplombe le vide de la
/// cage.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Stair {
    /// La case du bas, celle où la volée commence.
    pub foot: (u32, u32, u32),
    /// Le côté vers lequel on monte.
    pub climb: Side,
}

impl Stair {
    /// La case du dessus, que la cage occupe aussi.
    ///
    /// Une cage fait deux étages de haut : au sommet de la volée, un plafond à
    /// hauteur d'étage ne laisserait pas de quoi passer. Cette case n'est donc
    /// pas une cellule à elle, et elle perd ses passages latéraux comme celle du
    /// bas.
    pub fn head(self) -> (u32, u32, u32) {
        (self.foot.0, self.foot.1, self.foot.2 + 1)
    }
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
    /// Les volées, une par paire d'étages consécutifs.
    stairs: Vec<Stair>,
}

impl Grid {
    /// Engendre un labyrinthe.
    ///
    /// **Les escaliers sont choisis avant le creusement**, et c'est la seule
    /// façon de tenir leur contrainte : une case qui porte une volée ne peut
    /// avoir de passages que dans l'axe de sa montée. L'imposer après coup
    /// déconnecterait — dans un arbre, toute arête est l'unique route vers
    /// quelque chose — et l'imposer pendant ferait tourner la chasse sans fin,
    /// une interdiction géométrique ne pouvant pas être une simple préférence.
    ///
    /// Les retirer du graphe avant de creuser rend au contraire la contrainte
    /// vraie par construction, sans un seul rejet.
    ///
    /// # Panique
    ///
    /// Si une dimension est nulle — un réglage que l'appelant écrit, pas une
    /// entrée qu'il reçoit.
    pub fn generate(settings: Settings) -> Self {
        let (width, height, levels) = settings.extent;
        assert!(
            width > 0 && height > 0 && levels > 0,
            "une grille sans case n'est pas un labyrinthe : {:?}",
            settings.extent
        );

        let span = (width as usize + 2) * (height as usize + 2) * (levels as usize + 2);
        let walls = vec![0u8; span];

        let mut grid = Self {
            extent: settings.extent,
            walls,
            start: (0, 0, 0),
            exit: (0, 0, 0),
            stairs: Vec::new(),
        };

        for z in 0..levels as i32 {
            for y in 0..height as i32 {
                for x in 0..width as i32 {
                    grid.put(x, y, z, INTACT);
                }
            }
        }

        let mut rng = Rng::new(settings.seed);
        grid.reserve(&mut rng, settings.stairs);

        // Le centre plutôt qu'un coin : le creusement y a ses six voisins, et la
        // chasse n'a donc rien à faire avant plusieurs dizaines de coups.
        let origin = ((width / 2) as i32, (height / 2) as i32, (levels / 2) as i32);
        grid.dig(origin, &mut rng);
        grid.climb();
        grid.braid(&mut rng, settings.loops);

        grid.start = (origin.0 as u32, origin.1 as u32, origin.2 as u32);
        grid.exit = grid.farthest(grid.start);
        grid
    }

    /// Les volées, une par paire d'étages consécutifs.
    pub fn stairs(&self) -> &[Stair] {
        &self.stairs
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

    /// Choisit une volée par paire d'étages consécutifs, et la perce.
    ///
    /// **Une seule par paire, et c'est une identité et non un réglage** : dans
    /// un arbre couvrant, le nombre de passages verticaux vaut `Σ morceaux
    /// d'étage − 1`. En vouloir deux découperait un étage en deux morceaux dont
    /// l'un ne s'atteindrait qu'en changeant de niveau. Ce qui en donne
    /// davantage sans découper, ce sont les boucles de `braid`.
    ///
    /// Le graphe quotient des étages est une chaîne — une case ne voisine que
    /// l'étage au-dessus et celui en dessous —, donc il n'y a aucun choix de
    /// topologie à faire : seulement un choix de position.
    fn reserve(&mut self, rng: &mut Rng, count: u32) {
        let levels = self.extent.2;
        if levels < 2 {
            return;
        }
        // Une par paire d'étages d'abord : c'est ce qui rend le labyrinthe d'un
        // seul tenant, et ça ne se négocie pas. Le graphe quotient des étages
        // est une chaîne, donc il n'y a aucune topologie à choisir — seulement
        // des positions.
        //
        // **Par jeux entiers et non volée par volée** : la connexité ne se juge
        // qu'une fois la chaîne complète — tant qu'il manque une paire, les
        // étages du dessus sont légitimement hors d'atteinte —, et la volée
        // fautive n'est pas forcément la dernière posée.
        let mut linked = false;
        for _ in 0..64 {
            self.stairs.clear();
            for level in 0..levels - 1 {
                self.place(rng, level);
            }
            linked = self.reaches_all();
            if linked {
                break;
            }
        }
        assert!(
            linked,
            "aucune position de volée ne laisse le labyrinthe d'un seul tenant : \
             il y faut au moins trois cases sur un des deux axes horizontaux, \
             pour que le pied et la tête aient chacun leur issue dans la grille"
        );

        // Les suivantes ouvrent chacune une boucle verticale, et chacune aveugle
        // deux cases de plus : celle qui déconnecte est écartée, ce qui en pose
        // moins que demandé plutôt que de rendre une carte en morceaux.
        for _ in levels - 1..count.max(levels - 1) {
            let level = rng.below(levels - 1);
            let before = self.stairs.len();
            self.place(rng, level);
            if self.stairs.len() > before && !self.reaches_all() {
                self.stairs.pop();
            }
        }
    }

    /// Vrai si le graphe que le creusement peut emprunter relie toutes les cases.
    ///
    /// Ce graphe n'est pas celui de la grille : il porte les arêtes horizontales
    /// qu'aucune volée n'aveugle, et **les seules volées obligatoires** — les
    /// surnuméraires se percent après le creusement, donc s'y fier validerait une
    /// connexité qu'il n'atteint pas.
    ///
    /// Sans ce contrôle, l'échec arrive dans `dig`, où la chasse finit les mains
    /// vides : c'est tard, et le message ne dit pas que la faute est au placement.
    fn reaches_all(&self) -> bool {
        let mut seen = vec![false; self.walls.len()];
        let mut stack = vec![(0i32, 0i32, 0i32)];
        seen[self.offset(0, 0, 0)] = true;
        let mut count = 1;

        while let Some(cell) = stack.pop() {
            for side in Side::ALL {
                if side.is_vertical() && !self.is_flight(cell, side) {
                    continue;
                }
                if self.blind(cell, side) {
                    continue;
                }
                let (dx, dy, dz) = side.step();
                let next = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
                if self.at(next.0, next.1, next.2) == 0 {
                    continue;
                }
                let offset = self.offset(next.0, next.1, next.2);
                if seen[offset] {
                    continue;
                }
                seen[offset] = true;
                count += 1;
                stack.push(next);
            }
        }
        count == self.count()
    }

    /// Pose une volée à un étage donné, sur une case que nulle autre n'occupe.
    ///
    /// Une cage tient deux cases superposées, donc deux volées ne peuvent pas se
    /// chevaucher ni s'empiler. Le tirage est borné : sur une grille trop petite
    /// pour le compte demandé, on en pose moins plutôt que de tourner.
    fn place(&mut self, rng: &mut Rng, level: u32) {
        let (width, height, _) = self.extent;
        for _ in 0..256 {
            // Les quatre orientations, et pas seulement les deux croissantes :
            // un immeuble a des cages dans tous les sens, et deux directions
            // laissées de côté sont deux chemins de l'export que rien
            // n'exécute.
            let stair = Stair {
                foot: (rng.below(width), rng.below(height), level),
                climb: Side::ALL[rng.below(4) as usize],
            };
            let busy = self.stairs.iter().any(|other| {
                let taken = [other.foot, other.head()];
                taken.contains(&stair.foot) || taken.contains(&stair.head())
            });
            if busy {
                continue;
            }
            // La volée n'est pas percée ici : elle est seulement **autorisée**.
            // La percer d'avance sèmerait des germes d'arbre que le creusement
            // ne relierait jamais entre eux — chaque germe grandirait de son
            // côté, et le labyrinthe sortirait en forêt sans que rien ne le
            // dise.
            self.stairs.push(stair);
            return;
        }
    }

    /// Vrai si ce mur est aveuglé par une volée, **d'un côté ou de l'autre**.
    ///
    /// Une cage n'a que **deux** ouvertures, et non deux par case : en bas celle
    /// par laquelle on entre, en haut celle par laquelle on sort. Les deux autres
    /// faces de l'axe de montée n'ont pas de volume derrière elles — côté montée
    /// en bas le sol est déjà arrivé à la cote de l'étage suivant, côté entrée en
    /// haut on surplombe le vide de la cage. Un passage percé là déboucherait sur
    /// un mur : le portail ne s'apparierait pas, ce qui est un mur et non une
    /// erreur, et la connexité du labyrinthe deviendrait fausse en silence.
    ///
    /// Les retirer du graphe **avant** de creuser est ce qui rend la contrainte
    /// vraie sans jamais avoir à rejeter un perçage.
    ///
    /// **Les deux côtés, et c'est tout le piège** : un mur se perce des deux
    /// faces à la fois, donc l'interdire d'un seul côté laisse la voisine
    /// l'ouvrir pour nous. D'où les deux formes du test — `side` depuis la case
    /// proche, son opposé depuis la lointaine.
    fn blind(&self, cell: (i32, i32, i32), side: Side) -> bool {
        if side.is_vertical() {
            return false;
        }
        let (dx, dy, dz) = side.step();
        let near = (cell.0 as u32, cell.1 as u32, cell.2 as u32);
        let far = (
            (cell.0 + dx) as u32,
            (cell.1 + dy) as u32,
            (cell.2 + dz) as u32,
        );
        self.stairs.iter().any(|stair| {
            let entry = stair.climb.facing();
            (near == stair.foot && side != entry)
                || (far == stair.foot && side.facing() != entry)
                || (near == stair.head() && side != stair.climb)
                || (far == stair.head() && side.facing() != stair.climb)
        })
    }

    /// Perce la grille entière depuis une case.
    ///
    /// Un tour de boucle perce exactement un mur. Les volées étant déjà percées,
    /// il en reste `cases − 1 − volées`, et la chasse se déroule *dans* un tour
    /// sans en consommer.
    fn dig(&mut self, origin: (i32, i32, i32), rng: &mut Rng) {
        let mut hunt = Hunt::new(self.extent);
        let mut cell = origin;

        for _ in 1..self.count() {
            let mut candidates = self.candidates(cell);
            if candidates.is_empty() {
                let Some(found) = hunt.next(self) else {
                    // L'invariant dit qu'une case frontière existe tant qu'il
                    // reste à percer. Si la chasse rend malgré tout les mains
                    // vides, c'est que les volées ont coupé le graphe en deux :
                    // trop d'escaliers pour la grille. Échouer franchement vaut
                    // mieux que rendre une forêt muette, qui se verrait des
                    // heures plus tard comme un décor en morceaux.
                    panic!("les volées réservées ont déconnecté la grille");
                };
                cell = found;
                candidates = self.candidates(cell);
            }

            let side = candidates[rng.below(candidates.len() as u32) as usize];
            self.open(cell, side);
            let (dx, dy, dz) = side.step();
            cell = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
        }
    }

    /// Perce les volées que le creusement n'a pas prises.
    ///
    /// Il n'en emprunte qu'autant qu'il lui en faut pour atteindre chaque étage
    /// — une par paire, puisque la première suffit. Les autres sont donc encore
    /// fermées, et les ouvrir ici est exactement ce qui donne **plusieurs cages
    /// sans découper un étage** : chacune ferme une boucle verticale au lieu
    /// d'isoler un morceau.
    fn climb(&mut self) {
        for index in 0..self.stairs.len() {
            let stair = self.stairs[index];
            if !self.has_wall(stair.foot, Side::Up) {
                continue;
            }
            let foot = (
                stair.foot.0 as i32,
                stair.foot.1 as i32,
                stair.foot.2 as i32,
            );
            self.open(foot, Side::Up);
        }
    }

    /// Ouvre des passages en plus de ceux de l'arbre.
    ///
    /// Chacun ferme un cycle, donc `count` passages surnuméraires donnent
    /// exactement `count` boucles : le nombre cyclomatique se règle par
    /// construction plutôt que par mesure. Un mur déjà percé ou aveuglé par une
    /// volée est passé sans consommer le compte, faute de quoi le total ne
    /// serait plus exact.
    fn braid(&mut self, rng: &mut Rng, count: u32) {
        let (width, height, levels) = self.extent;
        let mut opened = 0;
        // Borné : sur une grille pleine, un tirage trouve un mur intérieur à
        // percer en quelques coups, mais rien ne garantit qu'il en reste autant
        // que demandé.
        for _ in 0..count * 64 {
            if opened == count {
                return;
            }
            let cell = (
                rng.below(width) as i32,
                rng.below(height) as i32,
                rng.below(levels) as i32,
            );
            let side = Side::ALL[rng.below(4) as usize];
            let (dx, dy, dz) = side.step();
            let next = (cell.0 + dx, cell.1 + dy, cell.2 + dz);
            if self.at(next.0, next.1, next.2) == 0 || self.blind(cell, side) {
                continue;
            }
            if !self.has_wall((cell.0 as u32, cell.1 as u32, cell.2 as u32), side) {
                continue;
            }
            self.open(cell, side);
            opened += 1;
        }
    }

    /// Vrai si ce côté est une volée que le creusement a le droit de prendre.
    ///
    /// Partout ailleurs, monter ferait un trou dans un plafond. Et **seules les
    /// volées obligatoires** — les `étages − 1` premières, que `reserve` pose en
    /// tête — lui sont ouvertes : s'il prenait aussi les autres, elles
    /// entreraient dans l'arbre, et un arbre à `k` passages verticaux a `k + 1`
    /// morceaux d'étage. Les surnuméraires se percent après lui, où elles
    /// ferment une boucle au lieu d'isoler un fragment.
    fn is_flight(&self, cell: (i32, i32, i32), side: Side) -> bool {
        let mandatory = self.extent.2.saturating_sub(1) as usize;
        let here = (cell.0 as u32, cell.1 as u32, cell.2 as u32);
        self.stairs.iter().take(mandatory).any(|stair| match side {
            Side::Up => here == stair.foot,
            Side::Down => here == stair.head(),
            _ => false,
        })
    }

    /// Les côtés d'une case qui mènent vers un voisin encore vierge.
    ///
    /// Un côté vertical n'y entre que s'il est une volée réservée, et une face
    /// aveuglée par une volée n'y entre jamais : c'est là toute la réservation,
    /// et elle tient l'invariant qui garantit la terminaison — tant qu'une case
    /// frontière a un candidat, elle en a un d'autorisé.
    fn candidates(&self, cell: (i32, i32, i32)) -> Vec<Side> {
        let mut out = Vec::with_capacity(5);
        for side in Side::ALL {
            if side.is_vertical() && !self.is_flight(cell, side) {
                continue;
            }
            if self.blind(cell, side) {
                continue;
            }
            let (dx, dy, dz) = side.step();
            if self.at(cell.0 + dx, cell.1 + dy, cell.2 + dz) == INTACT {
                out.push(side);
            }
        }
        out
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
    ///
    /// **Borné à une révolution**, et rien de moins : cet argument vaut tant que
    /// chaque case frontière garde un côté autorisé. Le jour où une interdiction
    /// le romprait, une boucle sans borne tournerait sans fin au lieu d'échouer
    /// franchement — et c'est un échec muet qu'on cherche des heures.
    fn next(&mut self, grid: &Grid) -> Option<(i32, i32, i32)> {
        let (width, height, levels) = self.extent;
        for _ in 0..width as usize * height as usize * levels as usize {
            self.advance();
            let cell = self.cell;
            if grid.at(cell.0, cell.1, cell.2) == INTACT {
                continue;
            }
            if !grid.candidates(cell).is_empty() {
                return Some(cell);
            }
        }
        None
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
