// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

// https://astro.build/config
export default defineConfig({
  site: "https://moonkraken.github.io/shore",
  integrations: [
    starlight({
      title: "My Docs",
      social: [{
        icon: "github",
        label: "GitHub",
        href: "https://moonkraken.github.io/shore",
      }],
      sidebar: [
        {
          label: "Guides",
          items: [
            // Each item here is one entry in the navigation menu.
            { label: "Example Guide", slug: "guides/example" },
          ],
        },
        {
          label: "Reference",
          autogenerate: { directory: "reference" },
        },
      ],
    }),
  ],
});
