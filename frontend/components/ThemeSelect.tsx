import { useState, useRef, useEffect } from "react";
import { Theme } from "../utils/settingsStorage";
import "./ThemeSelect.css";

interface ThemeSelectProps {
  value: Theme;
  onChange: (value: Theme) => void;
  id?: string;
  className?: string;
}

export default function ThemeSelect({ value, onChange, id, className = "" }: ThemeSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const options: { value: Theme; label: string }[] = [
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
    { value: "system", label: "System" },
  ];

  const selectedOption = options.find(opt => opt.value === value) || options[0];

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (dropdownRef.current && !dropdownRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("mousedown", handleClickOutside);
    }

    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [isOpen]);

  const handleSelect = (optionValue: Theme) => {
    onChange(optionValue);
    setIsOpen(false);
  };

  return (
    <div className={`theme-select-container ${className}`} ref={dropdownRef}>
      <button
        id={id}
        type="button"
        className="theme-select-button"
        onClick={() => setIsOpen(!isOpen)}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
      >
        <span>{selectedOption.label}</span>
        <span className={`theme-select-arrow ${isOpen ? "open" : ""}`}>▼</span>
      </button>
      {isOpen && (
        <div className="theme-select-dropdown">
          {options.map((option) => (
            <button
              key={option.value}
              type="button"
              className={`theme-select-option ${value === option.value ? "selected" : ""}`}
              onClick={() => handleSelect(option.value)}
              role="option"
              aria-selected={value === option.value}
            >
              {option.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

