export interface BaseMod {
  modId: string;
  modPath: string;
  folder?: string;
  details?: WorkshopFileDetails;
  updated?: boolean;
  nonSteamMod?: boolean;
  previewImagePath?: string;
}

export interface WorkshopFileDetails {
  publishedfileid: string;
  result: number;
  creator: string;
  creator_app_id: number;
  consumer_app_id: number;
  filename: string;
  file_size: number;
  file_url: string;
  hcontent_file: string;
  preview_url: string;
  hcontent_preview: string;
  title: string;
  description: string;
  time_created: number;
  time_updated: number;
  visibility: number;
  flags: number;
  workshop_file_url: string;
  workshop_accepted: boolean;
  show_subscribe_all: boolean;
  num_comments_developer: number;
  num_comments_public: number;
  banned: boolean;
  ban_reason: string;
  banner: string;
  can_be_deleted: boolean;
  app_name: string;
  file_type: number;
  can_subscribe: boolean;
  subscriptions: number;
  favorited: number;
  followers: number;
  lifetime_subscriptions: number;
  lifetime_favorited: number;
  lifetime_followers: number;
  lifetime_playtime: string;
  lifetime_playtime_sessions: string;
  views: number;
  num_children: number;
  num_reports: number;
  tags: Array<{ tag: string }>;
}

