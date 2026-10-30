# KidLearn (working title)

A learning app for little kids (elementary school, grades 1–4). Kids browse
learning categories shared by other users, or describe what they want to learn
and get an AI-generated course: ordered subcategories with small
Duolingo-style multiple-choice questions, with pictures.

## How it works

1. The user signs up / logs in (Google OAuth planned; a dev-login button
   exists for development).
2. They browse public categories, or create their own (public or private) by
   describing the subject in their own language (e.g. a Slovak kid types
   "prvouka").
3. The backend asks the Meta Model API (`api.meta.ai`) for N subcategories (default 20,
   configurable) in pedagogical order, then generates 5–10 questions per
   subcategory. Questions reference image assets; SVGs are generated on demand.
4. The kid opens a category, sees the ordered subcategories, and learns
   through them with Duolingo-like progress tracking.

## Layout

- `backend/` — Rust (Axum) API: auth, categories, Meta AI generation, SQLite.
- `app/` — Expo (React Native + Web) client from one codebase: web app and
  native mobile apps.
- `backend/prompts/` — LLM prompt templates (planner, questions, SVG art).

## Quickstart

Backend:

```sh
cd backend
cp .env.example .env   # set META_API_KEY to enable generation
cargo run
# API on http://localhost:3000, health at /health
```

App (web):

```sh
cd app
npm install
npx expo start --web
# open http://localhost:8081 in a browser
```

App (phone): install Expo Go, run `npx expo start`, scan the QR code.

## Environment

| Variable        | Default                              | Purpose                          |
| --------------- | ------------------------------------ | -------------------------------- |
| `DATABASE_URL`  | `sqlite://kidlearn.db`               | SQLite database                  |
| `JWT_SECRET`    | `dev-secret-change-me`               | Token signing (dev default)      |
| `META_API_BASE` | `https://api.meta.ai/v1`             | Meta Model API endpoint          |
| `META_API_KEY`  | _(none)_                             | Enables AI generation            |
| `META_MODEL`    | `muse-spark-1.3`                     | Generation model                 |

## Roadmap

- Google OAuth (replace dev login in production)
- Background generation jobs with live progress
- Duolingo-style lesson runner + progress/XP
- Public category gallery with search
- More UI languages (English + Slovak first)
