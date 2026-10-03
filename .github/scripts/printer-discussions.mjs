// Keeps one discussion per printer profile in the "Printers" category: a 👍 on it means "this printed
// fine for me", a 👎 and a comment mean it didn't. The website counts those reactions to light up
// its printer map, and finds each printer's discussion by the marker comment at the top of the body.
//
// Creates the discussion for a new profile, and rewrites the title and body when the profile's name
// changes. Never deletes: a removed profile keeps its discussion and its history.
//
// Usage: GITHUB_TOKEN=... node .github/scripts/printer-discussions.mjs [--dry-run]
// The category has to exist first (Settings › Discussions; API can't create one), in the
// Announcement format so that only maintainers open threads in it and everybody else can react.
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const [OWNER, NAME] = (process.env.GITHUB_REPOSITORY ?? 'vvvlladimir/Encrust').split('/');
const CATEGORY = process.env.CATEGORY ?? 'printers';
const DIR = 'assets/profiles/printers';
const DRY = process.argv.includes('--dry-run');
const TOKEN = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;
if (!TOKEN) throw new Error('GITHUB_TOKEN is not set');

const MARKER = /<!-- encrust-printer: ([a-z0-9-]+) -->/;
const str = (src, key) => src.match(new RegExp(`^${key}\\s*=\\s*"([^"]*)"`, 'm'))?.[1];

async function gql(query, variables = {}) {
  const res = await fetch('https://api.github.com/graphql', {
    method: 'POST',
    headers: { authorization: `bearer ${TOKEN}`, 'content-type': 'application/json' },
    body: JSON.stringify({ query, variables }),
  });
  const json = await res.json();
  if (!res.ok || json.errors) throw new Error(JSON.stringify(json.errors ?? json));
  return json.data;
}

const profiles = readdirSync(DIR)
  .filter((f) => f.endsWith('.toml'))
  .map((f) => {
    const src = readFileSync(join(DIR, f), 'utf8');
    const slug = f.replace(/\.toml$/, '');
    return { slug, title: `${str(src, 'manufacturer')} ${str(src, 'name')}` };
  });

const body = ({ slug, title }) => `<!-- encrust-printer: ${slug} -->
**Printed something on the ${title} with Encrust?**

- **It worked:** give this post a 👍. That's all it takes, one per person.
- **It didn't:** a 👎, and a comment with what went wrong: the Encrust version, the resin, and what came off the plate.

The more 👍 a printer has, the brighter it shows on the Encrust website's printer map. The numbers behind it are in [the profile](https://github.com/${OWNER}/${NAME}/blob/main/${DIR}/${slug}.toml); if one of them is wrong, [say so in an issue](https://github.com/${OWNER}/${NAME}/issues/new?template=printer-profile.yml).
`;

const { repository: repo } = await gql(
  `query($owner: String!, $name: String!) {
    repository(owner: $owner, name: $name) { id discussionCategories(first: 50) { nodes { id slug } } }
  }`,
  { owner: OWNER, name: NAME },
);
const category = repo.discussionCategories.nodes.find((c) => c.slug === CATEGORY);
if (!category) throw new Error(`No discussion category "${CATEGORY}" in ${OWNER}/${NAME}: create it first (Announcement format)`);

const existing = new Map();
for (let after = null; ; ) {
  const { repository } = await gql(
    `query($owner: String!, $name: String!, $category: ID!, $after: String) {
      repository(owner: $owner, name: $name) {
        discussions(first: 100, after: $after, categoryId: $category) {
          nodes { id number title body }
          pageInfo { hasNextPage endCursor }
        }
      }
    }`,
    { owner: OWNER, name: NAME, category: category.id, after },
  );
  for (const d of repository.discussions.nodes) {
    const slug = d.body.match(MARKER)?.[1];
    if (slug) existing.set(slug, d);
  }
  if (!repository.discussions.pageInfo.hasNextPage) break;
  after = repository.discussions.pageInfo.endCursor;
}

let created = 0;
let updated = 0;
for (const p of profiles) {
  const d = existing.get(p.slug);
  const text = body(p);
  if (!d) {
    console.log(`create  ${p.slug}`);
    if (!DRY) {
      await gql(
        `mutation($repo: ID!, $category: ID!, $title: String!, $body: String!) {
          createDiscussion(input: { repositoryId: $repo, categoryId: $category, title: $title, body: $body }) { discussion { number } }
        }`,
        { repo: repo.id, category: category.id, title: p.title, body: text },
      );
      // GitHub's secondary rate limit punishes bursts of new content.
      await new Promise((r) => setTimeout(r, 1500));
    }
    created++;
  } else if (d.title !== p.title || d.body.trim() !== text.trim()) {
    console.log(`update  ${p.slug} (#${d.number})`);
    if (!DRY) {
      await gql(
        `mutation($id: ID!, $title: String!, $body: String!) {
          updateDiscussion(input: { discussionId: $id, title: $title, body: $body }) { discussion { number } }
        }`,
        { id: d.id, title: p.title, body: text },
      );
    }
    updated++;
  }
}
console.log(`${profiles.length} profiles: ${created} created, ${updated} updated${DRY ? ' (dry run)' : ''}`);
