import { ReactNode } from "react";
import "./SettingField.css";

interface SettingFieldProps {
  title: string;
  description: string;
  children: ReactNode;
  error?: string;
  success?: string;
}

export default function SettingField({ title, description, children, error, success }: SettingFieldProps) {
  return (
    <div className="setting-field-container">
      <div className="setting-field-info">
        <h3 className="setting-field-title">{title}</h3>
        <p className="setting-field-description">{description}</p>
      </div>
      <div className="setting-field-control">
        {children}
        {error && (
          <div className="setting-field-error">{error}</div>
        )}
        {success && (
          <div className="setting-field-success">{success}</div>
        )}
      </div>
    </div>
  );
}

