export const useFormatting = () => {
  const formatSize = (bytes: number | undefined): string => {
    if (!bytes || bytes === 0 || isNaN(bytes) || !isFinite(bytes)) return "Unknown";
    if (bytes < 1024) return Math.round(bytes) + " B";
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(2) + " KB";
    if (bytes < 1024 * 1024 * 1024) return (bytes / (1024 * 1024)).toFixed(2) + " MB";
    return (bytes / (1024 * 1024 * 1024)).toFixed(2) + " GB";
  };

  const formatDate = (date: Date | string | number | undefined): string => {
    if (!date) return "Unknown";
    let d: Date;
    if (typeof date === "number") {
      d = new Date(date * 1000); // Steam Workshop uses Unix timestamp
    } else if (typeof date === "string") {
      d = new Date(date);
    } else {
      d = date;
    }
    return d.toLocaleDateString() + " " + d.toLocaleTimeString();
  };

  return { formatSize, formatDate };
};

