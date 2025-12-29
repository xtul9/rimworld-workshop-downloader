import { BaseMod } from "../types";

export type SortBy = "name" | "date";
export type SortOrder = "asc" | "desc";

/**
 * Sort mods according to the specified criteria
 * 
 * Non-steam mods are always sorted by name (but still respect asc/desc order)
 * @param mods - Array of mods to sort
 * @param sortBy - Sort by "name" or "date"
 * @param sortOrder - Sort order "asc" or "desc"
 * @returns Sorted array of mods
 */
export function sortMods(
  mods: BaseMod[],
  sortBy: SortBy,
  sortOrder: SortOrder
): BaseMod[] {
  return [...mods].sort((a, b) => {
    const isANonSteam = a.nonSteamMod || false;
    const isBNonSteam = b.nonSteamMod || false;
    
    // Non-steam mods are always sorted by name (but respect asc/desc)
    if (isANonSteam || isBNonSteam) {
      // If both are non-steam, sort by name
      if (isANonSteam && isBNonSteam) {
        const nameA = a.details?.title || a.folder || a.modId || "";
        const nameB = b.details?.title || b.folder || b.modId || "";
        const comparison = nameA.localeCompare(nameB, undefined, { sensitivity: "base" });
        return sortOrder === "asc" ? comparison : -comparison;
      }
      // Non-steam mods come after Steam mods (or before, depending on sort order)
      // For consistency, we'll put non-steam mods at the end when sorting by date
      if (sortBy === "date") {
        return isANonSteam ? 1 : -1; // Non-steam mods go to the end
      }
      // When sorting by name, mix them together
      const nameA = a.details?.title || a.folder || a.modId || "";
      const nameB = b.details?.title || b.folder || b.modId || "";
      const comparison = nameA.localeCompare(nameB, undefined, { sensitivity: "base" });
      return sortOrder === "asc" ? comparison : -comparison;
    }
    
    // Both are Steam mods - use normal sorting logic
    if (sortBy === "name") {
      const nameA = a.details?.title || a.folder || a.modId || "";
      const nameB = b.details?.title || b.folder || b.modId || "";
      const comparison = nameA.localeCompare(nameB, undefined, { sensitivity: "base" });
      return sortOrder === "asc" ? comparison : -comparison;
    } else {
      // Sort by date (time_updated)
      // Steam mods should always have time_updated
      const dateA = a.details?.time_updated ?? 0;
      const dateB = b.details?.time_updated ?? 0;
      const comparison = dateA - dateB;
      
      // If dates are equal, use folder name as secondary sort criterion for stable sorting
      if (comparison === 0) {
        const nameA = a.details?.title || a.folder || a.modId || "";
        const nameB = b.details?.title || b.folder || b.modId || "";
        const nameComparison = nameA.localeCompare(nameB, undefined, { sensitivity: "base" });
        return sortOrder === "asc" ? nameComparison : -nameComparison;
      }
      
      return sortOrder === "asc" ? comparison : -comparison;
    }
  });
}

