# Publishing a clip — the traps, in the order they will bite you

## 0. THE HARD RULE: the publish gate is an ANONYMOUS fetch

**A clip is published when a logged-out visitor can download and play it. Nothing
else counts.** Every check must be made with no cookie, no `gh auth`, no
credential of any kind:

```bash
curl -s -o /tmp/c.mp4 -w '%{http_code}\n' -L "$URL"   # must be 200
ffprobe -v error -show_entries format=duration -of csv=p=0 /tmp/c.mp4
```

Status code alone is not enough — fetch the bytes and probe them. A `302` only
says GitHub was willing to sign a redirect.

**This rule exists because we broke it and shipped 19 clips, 18 of which were
invisible to the entire world.** On 2026-08-21 an anonymous fetch of all 19
published embeds returned:

```
anon=404  auth=302     18 clips
anon=302  auth=302      1 clip  (203072)
```

The assets existed, the repo was public, the READMEs were committed and pushed,
the embeds were correctly formed, and every clip played perfectly **for us**,
because every verification we had ever run carried vjeux's session cookie. The
pipeline was built end to end behind a credential, so the one failure mode it
could not detect was the only one that mattered. A gate that runs with
credentials is not a gate.

`tools/ship-clip.sh` now performs this fetch itself and refuses to report
success on anything but 200 + playable bytes.

## 1. A pushed commit does NOT make an attachment public

This is the mechanism behind trap 0, and it is the opposite of what we believed.
The old note here read: *"a fresh asset 404s publicly until a commit references
it — it becomes public when a README referencing it lands."* **That is false.**
All 19 READMEs landed on the default branch of a public repo (`origin/main ==
HEAD`, 0 ahead / 0 behind) and 18 of the 19 assets stayed 404 to the public.

What actually authorises an asset is a reference in content **GitHub itself
renders when the content is saved** — a release body, an issue or PR comment, a
web-editor save. A plain `git push` of markdown never triggers it: the bytes
arrive over git, and nothing in GitHub's attachment service is told the asset is
now referenced by public content.

Proven directly, on an asset that had been 404 for hours:

```
before:  anon=404
         (append the asset URL to the videos-v1 release body)
after:   anon=302, then 200 with playable bytes — within seconds
```

Applied to all 18, then re-verified one at a time: **19 of 19 now download
anonymously and probe as valid MP4s.**

So the `videos-v1` release body carries a listing of every inline asset URL,
inside a `<details>` block. **That listing is load-bearing. Deleting it may take
every inline player in the repo dark again.**

One thing remains unexplained: why 203072 was public without such a reference.
No issue, PR, or web-flow commit mentions it — every commit in the repo has a
plain committer, none came through GitHub's web editor. Time alone is not the
answer: the asset I tested had been sitting for hours at 404 and flipped only
when it was referenced. Treat 203072 as an unexplained exception, not as
evidence that pushing is sometimes enough.

## 2. Never hash a file while it is still downloading

`curl` returning is not the same as the bytes being on disk and settled. Hashing
a partially-written file gives a short size and a wrong md5, which then reads as
a *transfer mismatch* — a real-looking failure caused entirely by measuring too
early. This happened twice in one session and nearly caused a good clip to be
withdrawn.

```bash
curl -s -L --retry 3 "$URL" -o "$F"
a=0; b=1; while [ "$a" != "$b" ]; do a=$(stat -f%z "$F"); sleep 1; b=$(stat -f%z "$F"); done
md5 -q "$F"      # only now
```

## 3. The attachment size limit is not a constant — do not predict it

One upload was rejected with S3 `EntityTooLarge`:

```
ProposedSize 22670023   MaxSizeAllowed 22667264
```

It is tempting to treat 22,667,264 as the cap and pre-emptively re-encode
anything larger. **That is wrong**: a **26,082,003-byte** file uploaded
successfully a few minutes later. Whatever the limit depends on, it is not a
fixed byte count.

So: **upload first, and re-encode only if the upload actually fails.** A
prediction here costs quality on files that would have gone through untouched.
The full-quality original should always go to the release asset regardless, so
the re-encode only ever affects the inline player.

## 4. The CSRF token is not a form input

This is what made every early attempt return 422. The token the drag-and-drop
upload needs lives in the **edit page's embedded JSON**, not in any `<input>`:

```
csrf_tokens['/upload/policies/assets'].post
```

Fetch any file's edit page (`https://github.com/<owner>/<repo>/edit/main/<path>`)
with the session cookie and read it from there. The three-request dance is then:

1. `POST /upload/policies/assets` → an S3 policy, an asset id, **and a second
   token** (the one step 3 needs — it is not the same token as step 1's)
2. `POST` the bytes to the S3 URL, using that policy's own form fields verbatim
3. `PUT /upload/assets/<id>` with the second token, to finalise

Finalising is what makes the asset *exist*. It is not what makes it *public* —
see trap 1.

## The general shape

Traps 2 and 3 are the same mistake: **a check that was wrong about data that was
right.** Trap 0 is its mirror image and the more dangerous one: **a check that
could not fail**, because the instrument was standing inside the thing it was
supposed to be testing. Before believing a green result, ask what a red one
would have looked like — and if you cannot answer, you do not have a check.
