import type { Participant } from "../generated/api.ts";

/** What the event form hands back when it is saved. */
export type EventValues = {
  title: string;
  description: string;
  when_text: string | null;
  scene_id: string | null;
  place_id: string | null;
  participants: Participant[];
};
