# LLM on CPU: model benchmark (part 5)

Server: Dell Latitude 5580, Intel Core i7-7820HQ (4 cores / 8 threads, AVX2, no AVX-512), 31 GiB RAM, CPU only in these runs. The machine also has an unused NVIDIA GeForce 940MX (Maxwell, nouveau driver) and the Intel HD 630; neither was tested (see DECISIONS).
llama.cpp tag `b11796` (commit `0a1ef30`), built with `GGML_NATIVE`, gcc 14.2. Scripts: `infra/scripts/part5/`. Measured 2026-10-27.
All three are mixture-of-experts models, Apache-2.0, files verified against the SHA-256 published by Hugging Face.

## Speed (tokens per second, `llama-bench`, one run each)
Prompt processing (pp) and generation (tg) are separate. Short prompt = 64 tokens, long prompt = 4096 tokens, 128 tokens generated.

| Model | File size | Threads | pp 64 | pp 4096 | tg 128 |
|---|---|---|---|---|---|
| Qwen3.6-35B-A3B | 20.6 GiB | 4 | 24.99 | 29.07 | 8.24 |
| Qwen3.6-35B-A3B | 20.6 GiB | 8 | 31.90 | 32.21 | 8.67 |
| Gemma 4 26B-A4B-it | 15.8 GiB | 4 | 23.74 | 22.01 | 7.05 |
| Gemma 4 26B-A4B-it | 15.8 GiB | 8 | 26.60 | 23.57 | 7.45 |
| gpt-oss-20b | 11.3 GiB | 4 | 24.89 | 22.60 | 10.14 |
| gpt-oss-20b | 11.3 GiB | 8 | 26.73 | 24.75 | 10.31 |

Reading it:
- Eight threads beat four slightly (about 5-28 %), so the service should use 8.
- Prompt processing is about 22-32 tokens/s whatever the prompt length, so a 4096-token prompt costs 2-3 minutes before the first word; a scene-sized prompt (a few hundred tokens) costs about 10-20 seconds. Later turns should keep prompts small and reuse the cached prefix.
- Generation is 7-10 tokens/s: a 400-word reply (about 600 tokens) takes roughly 1-1.5 minutes. Fine for overnight and background jobs, slow for live chat.
- Limits of these numbers: one run per setting, no concurrent load, CPU temperature not logged.

## Samples (end to end through `llama-server`, 4 threads, temperature 0.7, max 400 tokens)
Same prompt per language (`infra/scripts/part5/samples/`): continue a short scene in close third person, about 200 words. Speeds measured during the sample:

| Model | Russian (pp / tg tok/s) | English (pp / tg tok/s) |
|---|---|---|
| Qwen3.6-35B-A3B | 27.8 / 7.9 | 28.9 / 8.1 |
| Gemma 4 26B-A4B-it | 24.0 / 6.8 | 24.1 / 6.9 |
| gpt-oss-20b | 24.6 / 9.4 | 24.4 / 9.5 |

The samples follow, unedited, for the owner to score on meaning, voice, imagery and Russian naturalness.

### Qwen3.6-35B-A3B
**Russian**

> Марта не слушала. Её взгляд, прилипший к линии горизонта, где небо сливалось с водой, не отрывался ни на секунду. Ветер, сырой и солёный, пробирался под воротник, заставляя её невольно прижимать локти к бокам. Письмо в кармане казалось тяжёлым, словно оно было сделано не из бумаги, а из свинца. Она чувствовала, как пальцы на левой руке непроизвольно сжимаются в кулак, ногти впиваются в ладонь, но она не двигалась.
>
> Мужчина замолчал, сплёвывая за борт, и отошёл, не удостоив её взглядом. Шум порта нарастал: гудели краны, скрипели канаты, кто-то ругался на ломаном английском. Марта вздохнула, коротко и резко, выпуская пар из лёгких. Ей нужно было решить, оставить письмо или отправить его прямо сейчас, пока паром всё ещё стоит у причала, пока есть шанс, что кто-то из экипажа знает адресата.
>
> Она медленно выдохнула, чувствуя, как холодное стекло поручня обжигает ладонь. В кармане шуршала бумага. Она знала, что если не отправит его сегодня, то больше не отправит никогда. Третий день тянулся как вечность. Она достала конверт, разглаживая загнутые углы, и посмотрела на адрес. Город за спиной всё ещё казался нарисованным, но письмо было реальным, горячим от её тела. Она подошла к почтовому ящику у входа в терминал, где дежурил уставший смотритель, и протянула ему конверт.

**English**

> Marta watched a gull snag a piece of trash from the wake, its wings beating hard against the damp air. The man’s voice droned on, a steady hum beneath the slap of water against the hull. She shifted her weight, the wool of her coat scratching her neck. The letter in her pocket felt heavy, not with paper, but with the silence it carried. She had read the address three times on the envelope, tracing the loop of her own name, but the body inside remained sealed.
>
> A sudden lurch threw her slightly forward. The ferry’s engine coughed, a wet, ragged sound, before settling into a lower, more strained rhythm. The man beside her stopped talking, wiping his hands on his trousers. Marta looked down at the churning grey below. The town on the horizon seemed to shrink further, the details blurring into the mist. She reached into her pocket, her fingers brushing the crisp edge of the paper. She didn’t pull it out. She just let her hand rest there, feeling the stiff corner against her thigh, waiting for the boat to move, waiting for the water to stop looking like a mistake.

### Gemma 4 26B-A4B-it
**Russian**

> Марта не слушала его. Она смотрела, как мелкая рябь разбивается о бетонный пирс, и чувствовала, как пальцы в кармане непроизвольно сжимают край плотной бумаги. Письмо было тяжелым, почти осязаемым, словно внутри лежала галька.
>
> Ветер усилился, принося запах гниющих водорослей и сырого железа. Мужчина рядом замолчал, вытер лоб рукавом старой куртки и сплюнул в воду. Снова воцарилась тишина, нарушаемая лишь глухим стуком волн о сваи. Марта поправила воротник, пытаясь удержать тепло, но холод уже пробрался под подкладку.
>
> На другом берегу, в туманной дымке, мелькнул силуэт портового крана. Он казался скелетом огромного зверя, замершего в ожидании. Марта подумала о том, что если паром так и не придет, ей придется вернуться в город, к пустой квартире и запаху застоявшегося чая. Она не знала, что написано в письме, но знала, что после прочтения мир перестанет быть таким же невнятным, как этот пейзаж.
>
> Вдали послышался низкий, надрывный гудок. Из серой мглы начал проступать темный силуэт судна. Мужчина рядом оживился, подался вперед, вглядываясь в туман, а Марта лишь крепче сжала письмо в кармане.

**English**

> The wind picked up, smelling of salt and diesel. Marta pulled her collar tighter. She watched a single buoy bobbing in the swell, a rhythmic, lonely movement that matched the thrum of the idling boat. The man continued his monologue, his voice thin against the wind, complaining about the cost of parts and the incompetence of the local mechanics. 
>
> She reached into her pocket. Her fingers brushed the corner of the envelope, the paper crisp and unforgiving through the lining. She didn't pull it out. Instead, she gripped the edge of the metal rail, her knuckles turning white against the rusted paint. 
>
> A spray of cold mist hit her face. She wiped it away with the back of her hand, leaving a smear of grit on her cheek. To her left, a seagull landed on a piling, shook its wings, and stared at her with a black, unblinking eye. The ferry’s horn sounded, a low, mournful blast that vibrated in her teeth. It was coming, but it was still far off, a dark shape cutting through the haze. She stayed still, waiting for the vibration of the deck beneath her feet.

### gpt-oss-20b
**Russian**

> Паром снова опоздал. Марта стояла у поручня, письмо, сложенное совсем маленьким, лежало в кармане пальто нераспечатанным уже третий день. За серой водой город казался нарисованным и наполовину стёртым. Стоявший рядом мужчина рассказывал никому в особенности, что мотор с весны так и не наладили.
>
> Марта повернула голову к нему, но его взгляд остался направлен на горизонт. Взгляд, который казался сквозным, но в то же время и холодным. Он не делал усилий, чтобы показать хоть какую‑то эмоцию; всё, что он мог, было перечисление фактов: «Три дня, три рейса, три разъехавшихся от меня пассажира». Слова звучали как сухой отчет, как будто он передаёт новости, а не выражает чувства.
>
> Далее зашёл старик, его шляпа отдаляла от головы, как отдаленная часть облака. Он сел на скамейку и, взяв чашку чая, поглядел на Мару. В его глазах мелькнула лёгкая тревога, но она была сдержана и нераспространена. Он тихо спросил: «Вы ведь в пути, Марта?»
>
> Она кивнула. Слово «да» было без эмоций, но в ней была решимость. Она держала письмо так же крепко, как держала свою надежду. В этот момент паром, наконец, прибыл. Заливная вода отразила его серую панель, и крик бортового капитана раздался по воде. Марта взяла письмо, вставила его

**English**

> The man’s voice was low, a rasp that matched the creak of the ferry’s hull. “You get a new one, and the old one goes to the docks,” he said, shrugging. Marta listened but did not answer. Her eyes stayed on the water, watching how the light bled across the waves, turning them into a line of silver that might have been a path to somewhere else.
>
> A child on the other side of the rail, holding a red kite, stared at Marta with wide eyes. The kite’s string tangled in a coil of rusted rail. Marta touched the letter in her pocket, feeling the paper’s texture under her fingertips. She had carried it for days, never daring to open it, as if the act would break something inside her.
>
> The ferry’s engines coughed and sputtered, then fell silent. The man turned, his face turned toward the horizon, as if expecting something to happen there. Marta’s hand slipped back into her coat. The letter stayed folded, a small secret between her and the sea. The ferry, still on its way, pulled the town into a quiet drift, the only sound the distant call of gulls and the sigh of the tide.

## Assistant's reading (not the decision)
- **Gemma 4 26B-A4B:** cleanest Russian of the three, natural phrasing, good concrete imagery, stays in the scene. Slightly slower generation than Qwen.
- **Qwen3.6-35B-A3B:** fastest prompt processing and good Russian, a few awkward phrasings and it rushes the plot (posts the letter). Biggest file (20.6 GiB), leaving about 9 GiB for the OS, LanguageTool and the Rust server.
- **gpt-oss-20b:** fastest generation and smallest, but the Russian sample repeats the prompt, has grammatical errors and an invented name, and ends mid-sentence; English is acceptable. Not suited to Russian prose.
- A single sample per language is thin evidence; the owner's judgement decides.

## Pick
Pending owner decision. Unchosen models are deleted only after the owner picks (`95-delete-model.sh`).
