use ed25519_dalek::{Keypair, PublicKey, SecretKey};
use rand::rngs::OsRng; // For generating keypairs
use hex; // For encoding public key to hex

#[derive(Debug)]
pub struct SimulatedUser {
    pub keypair: Keypair,
    pub public_key_hex: String,
}

impl SimulatedUser {
    pub fn new() -> Self {
        let mut csprng = OsRng{};
        let keypair = Keypair::generate(&mut csprng);
        let public_key_hex = hex::encode(keypair.public.to_bytes());
        SimulatedUser {
            keypair,
            public_key_hex,
        }
    }
}

#[derive(Debug)]
pub struct UserManager {
    users: Vec<SimulatedUser>,
    current_idx: std::cell::Cell<usize>, // For round-robin user selection
}

impl UserManager {
    pub fn new(num_users: usize) -> Self {
        if num_users == 0 {
            panic!("Number of simulated users must be greater than 0.");
        }
        let users = (0..num_users).map(|_| SimulatedUser::new()).collect();
        UserManager {
            users,
            current_idx: std::cell::Cell::new(0),
        }
    }

    pub fn get_user_count(&self) -> usize {
        self.users.len()
    }

    /// Gets a user in a round-robin fashion.
    pub fn get_next_user(&self) -> &SimulatedUser {
        let idx = self.current_idx.get();
        let user = &self.users[idx];
        self.current_idx.set((idx + 1) % self.users.len());
        user
    }

    /// Gets a random user.
    /// Note: This might be less useful if we want to ensure all users participate somewhat evenly.
    /// `get_next_user` is generally preferred for distributing activity.
    #[allow(dead_code)]
    pub fn get_random_user(&self) -> &SimulatedUser {
        use rand::seq::SliceRandom;
        let mut rng = rand::thread_rng();
        self.users.choose(&mut rng).expect("User list should not be empty")
    }

    /// Gets a specific user by index.
    #[allow(dead_code)]
    pub fn get_user_by_index(&self, index: usize) -> Option<&SimulatedUser> {
        self.users.get(index)
    }
}

// Basic tests for UserManager
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_user_manager() {
        let manager = UserManager::new(5);
        assert_eq!(manager.get_user_count(), 5);
        for i in 0..5 {
            assert!(manager.get_user_by_index(i).is_some());
            assert!(!manager.get_user_by_index(i).unwrap().public_key_hex.is_empty());
        }
    }

    #[test]
    #[should_panic]
    fn test_create_user_manager_zero_users() {
        UserManager::new(0);
    }

    #[test]
    fn test_get_next_user() {
        let manager = UserManager::new(3);
        let pk1 = manager.get_next_user().public_key_hex.clone();
        let pk2 = manager.get_next_user().public_key_hex.clone();
        let pk3 = manager.get_next_user().public_key_hex.clone();
        let pk4 = manager.get_next_user().public_key_hex.clone();

        assert_ne!(pk1, pk2);
        assert_ne!(pk1, pk3);
        assert_ne!(pk2, pk3);
        assert_eq!(pk1, pk4); // Should wrap around
    }
}
```
