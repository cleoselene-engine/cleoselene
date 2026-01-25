use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

// --- Estruturas para A* ---

#[derive(Copy, Clone, PartialEq)]
struct State {
    f_score: f32,
    node_id: u64,
}

// Implementar Ord para BinaryHeap (Min-Heap simulado invertendo a comparação)
impl Eq for State {}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        // Inverte para que o MENOR custo fique no topo
        other
            .f_score
            .partial_cmp(&self.f_score)
            .unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

struct Node {
    x: f32,
    y: f32,
    edges: Vec<u64>, // IDs dos vizinhos
}

pub struct Graph {
    nodes: HashMap<u64, Node>,
}

impl Graph {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    pub fn add_node(&mut self, id: u64, x: f32, y: f32) {
        // Se já existe, atualiza? Ou ignora? Vamos assumir overwrite/init.
        self.nodes.entry(id).or_insert(Node {
            x,
            y,
            edges: Vec::new(),
        });
    }

    pub fn add_edge(&mut self, from: u64, to: u64) {
        // Grafo não-direcionado ou direcionado? O Lua manda A->B e B->A geralmente.
        // Vamos assumir direcionado conforme a chamada. O Lua chama add_edge(u, v) e add_edge(v, u).
        if let Some(node) = self.nodes.get_mut(&from) {
            // Evita duplicatas
            if !node.edges.contains(&to) {
                node.edges.push(to);
            }
        }
    }

    fn heuristic(&self, a: u64, b: u64) -> f32 {
        let n1 = &self.nodes[&a];
        let n2 = &self.nodes[&b];
        let dx = n1.x - n2.x;
        let dy = n1.y - n2.y;
        (dx * dx + dy * dy).sqrt()
    }

    fn dist(&self, a: u64, b: u64) -> f32 {
        // Custo real da aresta (Distância Euclidiana)
        // Assumindo que se tem aresta, calcula direto.
        self.heuristic(a, b)
    }

    pub fn find_path(&self, start: u64, goal: u64) -> Option<Vec<u64>> {
        if !self.nodes.contains_key(&start) || !self.nodes.contains_key(&goal) {
            return None;
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<u64, u64> = HashMap::new();
        let mut g_score: HashMap<u64, f32> = HashMap::new();

        // Init
        g_score.insert(start, 0.0);
        open_set.push(State {
            f_score: self.heuristic(start, goal),
            node_id: start,
        });

        while let Some(State {
            f_score: _,
            node_id: current,
        }) = open_set.pop()
        {
            if current == goal {
                // Reconstruir Caminho
                let mut path = Vec::new();
                let mut curr = goal;
                path.push(curr);
                while let Some(&prev) = came_from.get(&curr) {
                    path.push(prev);
                    curr = prev;
                }
                path.reverse();
                return Some(path);
            }

            // Para cada vizinho
            if let Some(node) = self.nodes.get(&current) {
                for &neighbor in &node.edges {
                    let tentative_g = g_score[&current] + self.dist(current, neighbor);

                    if tentative_g < *g_score.get(&neighbor).unwrap_or(&f32::INFINITY) {
                        came_from.insert(neighbor, current);
                        g_score.insert(neighbor, tentative_g);
                        let f = tentative_g + self.heuristic(neighbor, goal);
                        open_set.push(State {
                            f_score: f,
                            node_id: neighbor,
                        });
                    }
                }
            }
        }

        None // Caminho não encontrado
    }
}
