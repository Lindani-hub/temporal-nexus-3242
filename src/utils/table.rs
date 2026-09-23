use colored::Colorize;
use solana_sdk::signature::Signature;
use tabled::{Tabled, settings::{object::Rows, style::{BorderColor, LineText}, Color, Border, Highlight, Padding}, Table};

#[derive(Tabled)]
pub struct TableData {
    pub key: String,
    pub value: String,
}

pub trait TableSectionTitle {
    fn section_title(&mut self, row: usize, title: &str);
}

impl TableSectionTitle for Table {
    fn section_title(&mut self, row: usize, title: &str) {
        let title_color = Color::try_from(" ".bold().black().on_white().to_string()).unwrap();
        self.with(Highlight::new(Rows::single(row)).color(BorderColor::default().top(Color::FG_WHITE)));
        self.with(Highlight::new(Rows::single(row)).border(Border::new().top('━')));
        self.with(LineText::new(title, Rows::single(row)).color(title_color.clone()));
        if row > 0 {
            self.modify(Rows::single(row - 1), Padding::new(1, 1, 0, 1));
        }
    }
}

#[derive(Clone, Tabled)]
pub struct SoloMiningData {
    #[tabled(rename = "Signature")]
    pub signature: String,
    #[tabled(rename = "Block")]
    pub block: String,
    #[tabled(rename = "Timestamp")]
    pub timestamp: String,
    #[tabled(rename = "Timing")]
    pub timing: String,
    #[tabled(rename = "Score")]
    pub difficulty: String,
    #[tabled(rename = "Base Reward")]
    pub base_reward: String,
    #[tabled(rename = "Boost Reward")]
    pub boost_reward: String,
    #[tabled(rename = "Total Reward")]
    pub total_reward: String,
    #[tabled(rename = "Status")]
    pub status: String,
}

impl SoloMiningData {
    pub fn fetching(sig: Signature) -> Self {
        Self {
            signature: sig.to_string(),
            block: "–".to_string(),
            timestamp: "–".to_string(),
            difficulty: "–".to_string(),
            base_reward: "–".to_string(),
            boost_reward: "–".to_string(),
            total_reward: "–".to_string(),
            timing: "–".to_string(),
            status: "Fetching".to_string(),
        }
    }

    pub fn failed() -> Self {
        Self {
            signature: "–".to_string(),
            block: "–".to_string(),
            timestamp: "–".to_string(),
            difficulty: "–".to_string(),
            base_reward: "–".to_string(),
            boost_reward: "–".to_string(),
            total_reward: "–".to_string(),
            timing: "–".to_string(),
            status: "Failed".bold().red().to_string(),
        }
    }
}


#[derive(Clone, Tabled)]
pub struct PoolMiningData {
    #[tabled(rename = "Signature")]
    pub signature: String,
    #[tabled(rename = "Block")]
    pub block: String,
    #[tabled(rename = "Timestamp")]
    pub timestamp: String,
    #[tabled(rename = "Timing")]
    pub timing: String,
    #[tabled(rename = "Score")]
    pub difficulty: String,
    #[tabled(rename = "Pool Base Reward")]
    pub base_reward: String,
    #[tabled(rename = "Pool Boost Reward")]
    pub boost_reward: String,
    #[tabled(rename = "Pool Total Reward")]
    pub total_reward: String,
    #[tabled(rename = "My Score")]
    pub my_difficulty: String,
    #[tabled(rename = "My Reward")]
    pub my_reward: String,
}
