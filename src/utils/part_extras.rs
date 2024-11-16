use screeps::Part;

pub trait PartExtras {
    fn single_char(&self) -> char;
}

impl PartExtras for Part {
    fn single_char(&self) -> char {
        match self {
            Part::Move => 'M',
            Part::Work => 'W',
            Part::Carry => 'C',
            Part::Attack => 'A',
            Part::RangedAttack => 'R',
            Part::Tough => 'T',
            Part::Heal => 'H',
            Part::Claim => 'L',
            _ => '?',
        }
    }
}