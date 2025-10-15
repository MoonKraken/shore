// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

// https://astro.build/config
export default defineConfig({
  site: "https://moonkraken.github.io",
  base: process.env.NODE_ENV === "production" ? "/shore" : "/",
  integrations: [
    starlight({
      title: "Shore",
      social: [{
        icon: "github",
        label: "GitHub",
        href: "https://github.com/MoonKraken/shore",
      }],
      sidebar: [
        {
          label: "Getting Started",
          autogenerate: { directory: "gettingstarted" },
        },
        {
          label: "Keybindings",
          autogenerate: { directory: "keybindings" },
        },
      ],
    }),
  ],
});
