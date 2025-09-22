//! Unit tests for cache functionality

use super::*;
use std::fs;
use tempfile::TempDir;

#[cfg(test)]
mod cache_tests {
    use super::*;

    #[test]
    fn test_league_settings_path() {
        let path = league_settings_path(2023, 12345);

        // Should end with the expected filename
        assert!(path
            .to_string_lossy()
            .ends_with("espn-ffl/league-settings_2023_12345.json"));

        // Should contain the cache directory structure
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("espn-ffl"));
    }

    #[test]
    fn test_league_settings_path_different_values() {
        let path1 = league_settings_path(2022, 54321);
        let path2 = league_settings_path(2023, 12345);

        // Different inputs should produce different paths
        assert_ne!(path1, path2);

        // Check specific filename components
        assert!(path1.to_string_lossy().contains("2022_54321"));
        assert!(path2.to_string_lossy().contains("2023_12345"));
    }

    #[test]
    fn test_try_read_to_string_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        let test_content = "Hello, World!";

        // Write test content to file
        fs::write(&file_path, test_content).unwrap();

        // Test reading the file
        let result = try_read_to_string(&file_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_content);
    }

    #[test]
    fn test_try_read_to_string_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("nonexistent.txt");

        // Test reading non-existent file
        let result = try_read_to_string(&file_path);
        assert!(result.is_none());
    }

    #[test]
    fn test_try_read_to_string_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_file.txt");

        // Create empty file
        fs::write(&file_path, "").unwrap();

        // Test reading empty file
        let result = try_read_to_string(&file_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_try_read_to_string_utf8_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("utf8_file.txt");
        let test_content = "Hello, 世界! 🌟";

        // Write UTF-8 content
        fs::write(&file_path, test_content).unwrap();

        // Test reading UTF-8 content
        let result = try_read_to_string(&file_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_content);
    }

    #[test]
    fn test_try_read_to_string_large_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_file.txt");
        let test_content = "x".repeat(10000); // 10KB of 'x' characters

        // Write large content
        fs::write(&file_path, &test_content).unwrap();

        // Test reading large file
        let result = try_read_to_string(&file_path);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_content);
    }

    #[test]
    fn test_write_string_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");
        let test_content = "Test content for new file";

        // Write to new file
        let result = write_string(&file_path, test_content);
        assert!(result.is_ok());

        // Verify content was written correctly
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, test_content);
    }

    #[test]
    fn test_write_string_overwrite_existing() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("existing_file.txt");
        let original_content = "Original content";
        let new_content = "New content that overwrites";

        // Create file with original content
        fs::write(&file_path, original_content).unwrap();

        // Overwrite with new content
        let result = write_string(&file_path, new_content);
        assert!(result.is_ok());

        // Verify content was overwritten
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, new_content);
        assert_ne!(read_content, original_content);
    }

    #[test]
    fn test_write_string_create_directories() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("level1")
            .join("level2")
            .join("test_file.txt");
        let test_content = "Content in nested directory";

        // Ensure parent directories don't exist yet
        assert!(!nested_path.parent().unwrap().exists());

        // Write to file in nested directory
        let result = write_string(&nested_path, test_content);
        assert!(result.is_ok());

        // Verify directories were created and content was written
        assert!(nested_path.exists());
        let read_content = fs::read_to_string(&nested_path).unwrap();
        assert_eq!(read_content, test_content);
    }

    #[test]
    fn test_write_string_empty_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_content.txt");

        // Write empty string
        let result = write_string(&file_path, "");
        assert!(result.is_ok());

        // Verify empty file was created
        assert!(file_path.exists());
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, "");
    }

    #[test]
    fn test_write_string_utf8_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("utf8_content.txt");
        let test_content = "Hello, 世界! 🚀🎉";

        // Write UTF-8 content
        let result = write_string(&file_path, test_content);
        assert!(result.is_ok());

        // Verify UTF-8 content was written correctly
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, test_content);
    }

    #[test]
    fn test_write_string_large_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_content.txt");
        let test_content = "Large content: ".to_string() + &"data".repeat(5000);

        // Write large content
        let result = write_string(&file_path, &test_content);
        assert!(result.is_ok());

        // Verify large content was written correctly
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, test_content);
    }

    #[test]
    fn test_roundtrip_write_read() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("roundtrip.txt");
        let test_content = "Roundtrip test content\nwith multiple lines\nand special chars: áéíóú";

        // Write content
        let write_result = write_string(&file_path, test_content);
        assert!(write_result.is_ok());

        // Read content back
        let read_result = try_read_to_string(&file_path);
        assert!(read_result.is_some());
        assert_eq!(read_result.unwrap(), test_content);
    }

    #[test]
    fn test_write_string_json_content() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.json");
        let json_content = r#"{
  "season": 2023,
  "league_id": 12345,
  "settings": {
    "scoring": {
      "passing_yards": 0.04,
      "passing_tds": 4.0
    }
  }
}"#;

        // Write JSON content
        let result = write_string(&file_path, json_content);
        assert!(result.is_ok());

        // Verify JSON content was written correctly
        let read_content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, json_content);

        // Verify it's valid JSON
        let _: serde_json::Value = serde_json::from_str(&read_content).unwrap();
    }

    #[test]
    fn test_cache_path_with_special_ids() {
        // Test with edge case values
        let path1 = league_settings_path(0, 0);
        let path2 = league_settings_path(9999, u32::MAX);

        assert!(path1.to_string_lossy().contains("0_0"));
        assert!(path2
            .to_string_lossy()
            .contains(&format!("9999_{}", u32::MAX)));
    }

    #[test]
    fn test_try_read_to_string_file_read_error() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("readonly_file.txt");

        // Create a file and write some content
        fs::write(&file_path, "test content").unwrap();

        // Make file unreadable (this simulates a read error)
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&file_path).unwrap().permissions();
            perms.set_mode(0o000); // No permissions
            fs::set_permissions(&file_path, perms).unwrap();

            // Should return None due to read error
            let result = try_read_to_string(&file_path);
            assert!(result.is_none());
        }

        #[cfg(not(unix))]
        {
            // On non-Unix systems, just test that the function works normally
            let result = try_read_to_string(&file_path);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_write_string_parent_dir_creation() {
        let temp_dir = TempDir::new().unwrap();
        let nested_path = temp_dir
            .path()
            .join("deeply")
            .join("nested")
            .join("file.txt");

        // Parent directories don't exist yet
        assert!(!nested_path.parent().unwrap().exists());

        // Should create parent directories and write file
        let result = write_string(&nested_path, "test content");
        assert!(result.is_ok());

        // Should have created the directories
        assert!(nested_path.parent().unwrap().exists());

        // Should have written the file
        assert!(nested_path.exists());

        // Content should be correct
        let content = fs::read_to_string(&nested_path).unwrap();
        assert_eq!(content, "test content");
    }
}
