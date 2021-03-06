use chashmap::CHashMap;
use std::{
    fmt,
    fs::File,
    io::{self, prelude::*},
    time::Duration,
};

pub struct Stats {
    pings_to_peers: CHashMap<String, Vec<Duration>>,
    transmissions_rates: CHashMap<String, Vec<Duration>>,
    window_size: usize,
    peer_id: String,
}

impl Stats {
    pub fn new(window_size: usize, peer_id: String) -> Self {
        Self {
            pings_to_peers: CHashMap::new(),
            transmissions_rates: CHashMap::new(),
            window_size,
            peer_id,
        }
    }

    pub fn save_to_file(&self, filename: &str) -> io::Result<()> {
        let mut file = File::create(filename)?;
        file.write_all(self.to_string().as_bytes())?;
        Ok(())
    }

    pub fn add_ping(&self, peer_id: String, rtt: Duration) {
        if !self.pings_to_peers.contains_key(&peer_id) {
            self.pings_to_peers.insert_new(peer_id.clone(), Vec::new())
        }
        self.pings_to_peers
            .get_mut(&peer_id)
            .expect("Failed to get peer entry")
            .push_lossy(rtt, self.window_size)
    }

    pub fn add_transmission(&self, peer_id: String, time: Duration, n_bytes: u32) {
        if !self.transmissions_rates.contains_key(&peer_id) {
            self.transmissions_rates
                .insert_new(peer_id.clone(), Vec::new())
        }
        self.transmissions_rates
            .get_mut(&peer_id)
            .expect("Failed to get peer entry")
            .push_lossy(
                //put transmission rate which is elapsed time per byte
                time / n_bytes,
                self.window_size,
            )
    }
}

#[test]
fn correctly_added_pings() {
    let stats = Stats::new(100, "1".to_string());
    stats.add_ping("2".to_string(), Duration::from_secs(1));
    stats.add_ping("2".to_string(), Duration::from_secs(2));
    stats.add_ping("3".to_string(), Duration::from_secs(1));
    assert_eq!(stats.pings_to_peers.len(), 2);
    let peer_2_pings = stats.pings_to_peers.get("2").unwrap();
    assert_eq!(peer_2_pings.len(), 2)
}

#[test]
fn correctly_added_transmissions() {
    let stats = Stats::new(100, "1".to_string());
    stats.add_transmission("2".to_string(), Duration::from_secs(1), 1);
    stats.add_transmission("2".to_string(), Duration::from_secs(2), 1);
    stats.add_transmission("3".to_string(), Duration::from_secs(1), 1);
    assert_eq!(stats.transmissions_rates.len(), 2);
    let peer_2_transmissions = stats.transmissions_rates.get("2").unwrap();
    assert_eq!(peer_2_transmissions.len(), 2)
}

fn durations_mean(durations: &Vec<Duration>) -> Option<Duration> {
    if durations.is_empty() {
        None
    } else {
        Some(
            durations
                .iter()
                .fold(Duration::from_secs(0), |acc, x| acc + *x)
                / durations.len() as u32,
        )
    }
}

#[test]
fn correct_durations_mean() {
    let durations = vec![
        Duration::from_secs(1),
        Duration::from_secs(3),
        Duration::from_secs(5),
    ];
    assert_eq!(durations_mean(&durations).unwrap(), Duration::from_secs(3));
}

fn durations_std_dev(durations: &Vec<Duration>) -> Option<Duration> {
    let mean = durations_mean(durations)?.as_secs_f64();
    Some(Duration::from_secs_f64(
        (durations
            .iter()
            .fold(0f64, |acc, x| acc + (x.as_secs_f64() - mean).powi(2))
            / (durations.len() as f64))
            .sqrt(),
    ))
}

#[test]
fn correct_durations_std_dev() {
    let durations = vec![
        Duration::from_secs(1),
        Duration::from_secs(3),
        Duration::from_secs(5),
    ];
    let epsilon = 0.01;
    let std_dev = durations_std_dev(&durations).unwrap().as_secs_f64();
    assert!((std_dev - 1.63).abs() < epsilon);
}

/// Durations mean error with confidence interval of 95%
/// For correct estimation `durations.len()` should be at least `30`.
fn durations_error_with_ci(durations: &Vec<Duration>) -> Option<Duration> {
    // Z-value for 95 percent confidence interval
    let z = 1.96;
    let std_dev = durations_std_dev(durations)?;
    Some(Duration::from_secs_f64(
        z * std_dev.as_secs_f64() / (durations.len() as f64).sqrt(),
    ))
}

impl fmt::Display for Stats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ping_by_peer: String = self
            .pings_to_peers
            .clone()
            .into_iter()
            .map(|(peer, durations)| {
                match (
                    durations_mean(&durations),
                    durations_error_with_ci(&durations),
                ) {
                    (Some(duration), Some(error)) => {
                        format!("{:?} {:?}±{:?}\n", peer, duration, error)
                    }
                    _ => format!("No ping data for peer {:?}", peer),
                }
            })
            .collect();

        let transmission_rate_by_peer: String = self
            .transmissions_rates
            .clone()
            .into_iter()
            .map(|(peer, durations)| {
                match (
                    durations_mean(&durations),
                    durations_error_with_ci(&durations),
                ) {
                    (Some(duration), Some(error)) => {
                        format!("{:?} {:?}±{:?} per byte\n", peer, duration, error)
                    }
                    _ => format!("No transmission data for peer {:?}", peer),
                }
            })
            .collect();
        write!(
            f,
            "{:?}\nPing mean for each peer:\n{}Transmission rate mean by peer:\n{}",
            self.peer_id, ping_by_peer, transmission_rate_by_peer
        )
    }
}

pub trait PushLossy<T> {
    fn push_lossy(&mut self, element: T, window_size: usize);
}

impl<T> PushLossy<T> for Vec<T> {
    fn push_lossy(&mut self, element: T, window_size: usize) {
        if self.len() >= window_size {
            self.remove(0);
        }
        self.push(element);
    }
}

#[test]
fn correct_push_lossy() {
    let mut vector = Vec::new();
    vector.push_lossy(1, 2);
    vector.push_lossy(2, 2);
    vector.push_lossy(3, 2);
    assert_eq!(vector, vec![2, 3]);
}
