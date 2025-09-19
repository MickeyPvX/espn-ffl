//! Unit tests for CLI types and conversions

use super::*;
use std::str::FromStr;

#[cfg(test)]
mod league_id_tests {
    use super::*;

    #[test]
    fn test_league_id_new() {
        let id = LeagueId::new(12345);
        assert_eq!(id.as_u32(), 12345);
    }

    #[test]
    fn test_league_id_display() {
        let id = LeagueId::new(98765);
        assert_eq!(id.to_string(), "98765");
    }

    #[test]
    fn test_league_id_from_str_valid() {
        let id = LeagueId::from_str("54321").unwrap();
        assert_eq!(id.as_u32(), 54321);
    }

    #[test]
    fn test_league_id_from_str_invalid() {
        let result = LeagueId::from_str("not_a_number");
        assert!(result.is_err());
    }

    #[test]
    fn test_league_id_from_str_negative() {
        let result = LeagueId::from_str("-123");
        assert!(result.is_err());
    }

    #[test]
    fn test_league_id_from_str_zero() {
        let id = LeagueId::from_str("0").unwrap();
        assert_eq!(id.as_u32(), 0);
    }

    #[test]
    fn test_league_id_serde() {
        let id = LeagueId::new(123);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "123");

        let deserialized: LeagueId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, id);
    }
}

#[cfg(test)]
mod player_id_tests {
    use super::*;

    #[test]
    fn test_player_id_new() {
        let id = PlayerId::new(9876543210);
        assert_eq!(id.as_u64(), 9876543210);
    }

    #[test]
    fn test_player_id_zero() {
        let id = PlayerId::new(0);
        assert_eq!(id.as_u64(), 0);
    }

    #[test]
    fn test_player_id_max_value() {
        let id = PlayerId::new(u64::MAX);
        assert_eq!(id.as_u64(), u64::MAX);
    }

    #[test]
    fn test_player_id_serde() {
        let id = PlayerId::new(1234567890);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "1234567890");

        let deserialized: PlayerId = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, id);
    }
}

#[cfg(test)]
mod season_tests {
    use super::*;

    #[test]
    fn test_season_new() {
        let season = Season::new(2023);
        assert_eq!(season.as_u16(), 2023);
    }

    #[test]
    fn test_season_default() {
        let season = Season::default();
        assert_eq!(season.as_u16(), 2025);
    }

    #[test]
    fn test_season_display() {
        let season = Season::new(2024);
        assert_eq!(season.to_string(), "2024");
    }

    #[test]
    fn test_season_from_str_valid() {
        let season = Season::from_str("2022").unwrap();
        assert_eq!(season.as_u16(), 2022);
    }

    #[test]
    fn test_season_from_str_invalid() {
        let result = Season::from_str("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_season_from_str_future() {
        let season = Season::from_str("2030").unwrap();
        assert_eq!(season.as_u16(), 2030);
    }

    #[test]
    fn test_season_serde() {
        let season = Season::new(2023);
        let json = serde_json::to_string(&season).unwrap();
        assert_eq!(json, "2023");

        let deserialized: Season = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, season);
    }
}

#[cfg(test)]
mod week_tests {
    use super::*;

    #[test]
    fn test_week_new() {
        let week = Week::new(15);
        assert_eq!(week.as_u16(), 15);
    }

    #[test]
    fn test_week_default() {
        let week = Week::default();
        assert_eq!(week.as_u16(), 1);
    }

    #[test]
    fn test_week_display() {
        let week = Week::new(8);
        assert_eq!(week.to_string(), "8");
    }

    #[test]
    fn test_week_from_str_valid() {
        let week = Week::from_str("12").unwrap();
        assert_eq!(week.as_u16(), 12);
    }

    #[test]
    fn test_week_from_str_invalid() {
        let result = Week::from_str("not_a_week");
        assert!(result.is_err());
    }

    #[test]
    fn test_week_zero() {
        let week = Week::from_str("0").unwrap();
        assert_eq!(week.as_u16(), 0);
    }

    #[test]
    fn test_week_large_number() {
        let week = Week::from_str("999").unwrap();
        assert_eq!(week.as_u16(), 999);
    }

    #[test]
    fn test_week_serde() {
        let week = Week::new(7);
        let json = serde_json::to_string(&week).unwrap();
        assert_eq!(json, "7");

        let deserialized: Week = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, week);
    }
}

#[cfg(test)]
mod position_tests {
    use super::*;

    #[test]
    fn test_position_from_str_standard() {
        assert_eq!(Position::from_str("QB").unwrap(), Position::QB);
        assert_eq!(Position::from_str("RB").unwrap(), Position::RB);
        assert_eq!(Position::from_str("WR").unwrap(), Position::WR);
        assert_eq!(Position::from_str("TE").unwrap(), Position::TE);
        assert_eq!(Position::from_str("K").unwrap(), Position::K);
        assert_eq!(Position::from_str("FLEX").unwrap(), Position::FLEX);
    }

    #[test]
    fn test_position_from_str_defense_aliases() {
        assert_eq!(Position::from_str("D").unwrap(), Position::D);
        assert_eq!(Position::from_str("D/ST").unwrap(), Position::D);
        assert_eq!(Position::from_str("DEF").unwrap(), Position::D);
        assert_eq!(Position::from_str("DST").unwrap(), Position::D);
    }

    #[test]
    fn test_position_from_str_case_insensitive() {
        assert_eq!(Position::from_str("qb").unwrap(), Position::QB);
        assert_eq!(Position::from_str("Rb").unwrap(), Position::RB);
        assert_eq!(Position::from_str("WR").unwrap(), Position::WR);
        assert_eq!(Position::from_str("te").unwrap(), Position::TE);
        assert_eq!(Position::from_str("flex").unwrap(), Position::FLEX);
    }

    #[test]
    fn test_position_from_str_invalid() {
        let result = Position::from_str("INVALID");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unrecognized player position"));
    }

    #[test]
    fn test_position_display() {
        assert_eq!(Position::QB.to_string(), "QB");
        assert_eq!(Position::RB.to_string(), "RB");
        assert_eq!(Position::WR.to_string(), "WR");
        assert_eq!(Position::TE.to_string(), "TE");
        assert_eq!(Position::K.to_string(), "K");
        assert_eq!(Position::D.to_string(), "D/ST");
        assert_eq!(Position::FLEX.to_string(), "FLEX");
    }

    #[test]
    fn test_position_to_u8() {
        assert_eq!(u8::from(Position::QB), 0);
        assert_eq!(u8::from(Position::RB), 2);
        assert_eq!(u8::from(Position::WR), 4);
        assert_eq!(u8::from(Position::TE), 6);
        assert_eq!(u8::from(Position::D), 16);
        assert_eq!(u8::from(Position::K), 17);
        assert_eq!(u8::from(Position::FLEX), 23);
    }

    #[test]
    fn test_position_try_from_u8_valid() {
        assert_eq!(Position::try_from(0).unwrap(), Position::QB);
        assert_eq!(Position::try_from(2).unwrap(), Position::RB);
        assert_eq!(Position::try_from(4).unwrap(), Position::WR);
        assert_eq!(Position::try_from(6).unwrap(), Position::TE);
        assert_eq!(Position::try_from(16).unwrap(), Position::D);
        assert_eq!(Position::try_from(17).unwrap(), Position::K);
        assert_eq!(Position::try_from(23).unwrap(), Position::FLEX);
    }

    #[test]
    fn test_position_try_from_u8_invalid() {
        let result = Position::try_from(99);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown Position ID: 99"));
    }

    #[test]
    fn test_position_roundtrip_conversion() {
        let positions = [
            Position::QB,
            Position::RB,
            Position::WR,
            Position::TE,
            Position::K,
            Position::D,
            Position::FLEX,
        ];

        for pos in positions {
            let id = u8::from(pos);
            let converted = Position::try_from(id).unwrap();
            assert_eq!(pos, converted);
        }
    }
}
