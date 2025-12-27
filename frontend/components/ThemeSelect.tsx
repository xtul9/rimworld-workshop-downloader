import { Theme } from "../utils/settingsStorage";
import Select, { SelectOption } from "./Select";

interface ThemeSelectProps {
  value: Theme;
  onChange: (value: Theme) => void;
  id?: string;
  className?: string;
}

export default function ThemeSelect({ value, onChange, id, className = "" }: ThemeSelectProps) {
  const options: SelectOption<Theme>[] = [
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
    { value: "system", label: "System" },
  ];

  return (
    <Select
      value={value}
      onChange={onChange}
      options={options}
      id={id}
      className={className}
    />
  );
}

