import { useState, useRef, useEffect } from "react";
import "./Select.css";

export interface SelectOption<T> {
  value: T;
  label: string;
}

interface SelectProps<T> {
  value: T;
  onChange: (value: T) => void;
  options: SelectOption<T>[];
  id?: string;
  className?: string;
  placeholder?: string;
}

export default function Select<T extends string | number>({ 
  value, 
  onChange, 
  options, 
  id, 
  className = "",
  placeholder
}: SelectProps<T>) {
  const [isOpen, setIsOpen] = useState(false);
  const dropdownRef = useRef<HTMLDivElement>(null);

  const selectedOption = options.find(opt => opt.value === value);

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

  const handleSelect = (optionValue: T) => {
    onChange(optionValue);
    setIsOpen(false);
  };

  return (
    <div className={`select-container ${className}`} ref={dropdownRef}>
      <button
        id={id}
        type="button"
        className="select-button"
        onClick={() => setIsOpen(!isOpen)}
        aria-expanded={isOpen}
        aria-haspopup="listbox"
      >
        <span>{selectedOption?.label || placeholder || "Select..."}</span>
        <span className={`select-arrow ${isOpen ? "open" : ""}`}>▼</span>
      </button>
      {isOpen && (
        <div className="select-dropdown">
          {options.map((option) => (
            <button
              key={String(option.value)}
              type="button"
              className={`select-option ${value === option.value ? "selected" : ""}`}
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

