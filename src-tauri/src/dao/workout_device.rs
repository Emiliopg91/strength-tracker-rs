use rusqlite_orm::Entity;

#[derive(Entity)]
#[entity("workout_device")]
#[primary_key(workout, device)]
#[index("device",(device))]
pub struct WorkoutDevice {
    pub workout: String,
    pub device: String,
    pub pending: bool,
}
