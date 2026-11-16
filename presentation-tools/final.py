import shutil, os
from PIL import Image, ImageDraw, ImageFont
OUT='/Users/damijancavar/development/kings-bounty/presentation-assets'
shutil.rmtree(OUT, ignore_errors=True); os.makedirs(OUT+'/extras')
F=lambda s,b=True: ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial%s.ttf'%(' Bold' if b else ''), s)
BG=(24,26,32)
pick={
 '01-world-map.png':'raw8/dbg-worldmap-revealed.png',
 '01b-overworld-start.png':'raw8/opening.png',
 '04-movement-before.png':'raw8/walk1-031.png',
 '04-movement.png':'raw8/walk1-033.png',
 '05-exploration.png':'raw8/chest-choice.png',
 '05b-exploration-signpost.png':'raw8/walk1-027.png',
 '05c-exploration-town-contract.png':'raw8/town-contract.png',
 '05d-exploration-castle-siege-prompt.png':'raw8/siege-prompt.png',
 '06-combat.png':'raw8/siege-r03.png',
 '06b-combat-later-stage.png':'raw8/siege-r08.png',
 '06c-combat-field-battle-obstacle.png':'raw8/fight1-start.png',
 '06d-combat-victory.png':'raw8/fight1-r07-Left.png',
 '06e-combat-defeat.png':'raw8/siege-end.png',
 '07-recruitment.png':'raw8/recruit-howmany.png',
 '07b-recruitment-after-purchase.png':'raw8/recruit-done.png',
 '07c-town-services-prices.png':'raw8/town-menu.png',
 '07d-army-overview.png':'raw8/army.png',
 '08-interface.png':'raw8/hero-view.png',
 '08b-interface-contract.png':'raw8/contract-view.png',
 '08c-interface-town-news.png':'raw8/town-news.png',
 '08d-interface-puzzle-map.png':'raw8/puzzle.png',
 '08e-interface-controls.png':'raw8/options.png',
 '09-gameplay.png':'raw8/walk1-023.png',
 '09b-gameplay.png':'raw8/walk2-010.png',
 'extras/title-screen.png':'raw8/title.png',
 'extras/logo-screen.png':'raw8/logo.png',
 'extras/credits.png':'raw8/credits.png',
 'extras/class-select.png':'raw8/class-select.png',
 'extras/difficulty-select.png':'raw8/difficulty.png',
 'extras/royal-hall.png':'raw8/royal-hall.png',
 'extras/siege-engines-bought.png':'raw8/town-siege-bought.png',
}
for k,v in pick.items(): shutil.copy(v, f'{OUT}/{k}')
for f in os.listdir('sheets'): shutil.copy('sheets/'+f, f'{OUT}/{f}')
shutil.copy('11-gameplay-demo.mp4', OUT)
# 10 evolution: four stages of the same opening scene
stages=[('old-724fec2/opening.png','1. Placeholder pixel art','22 Oct 2026 (git 724fec2)'),
        ('old-792f885/opening.png','2. Painted terrain, objects and hero','23 Oct 2026 (git 792f885)'),
        ('old-b8b9c70/opening.png','3. Continents re-shaped: mountain walls, shores','24 Oct 2026 (git b8b9c70)'),
        ('raw8/opening.png','4. Today: painted frame, sidebar and icons','current build')]
W,H,head,lab=1280,800,150,90
S=Image.new('RGB',(2*W+72,head+2*(H+lab)+24),BG); d=ImageDraw.Draw(S)
d.text((24,28),'Polish: one scene, four art passes',font=F(56),fill=(235,235,235))
d.text((24,96),'The same starting position (King\'s castle, chest, lake) rendered by the game from each version of the art in git history',font=F(28,False),fill=(160,165,175))
for i,(p,t,s) in enumerate(stages):
    im=Image.open(p).convert('RGB').resize((W,H),Image.BOX)
    x=24+(i%2)*(W+24); y=head+(i//2)*(H+lab)
    S.paste(im,(x,y)); d.text((x,y+H+10),t,font=F(34),fill=(232,190,80)); d.text((x,y+H+52),s,font=F(26,False),fill=(160,165,175))
S.save(f'{OUT}/10-polish-comparison.png'); print('10',S.size)
# 10c class select before/after
S=Image.new('RGB',(2*W+72,head+H+lab+24),BG); d=ImageDraw.Draw(S)
d.text((24,28),'Polish: the hero selection screen',font=F(56),fill=(235,235,235))
d.text((24,96),'Left: placeholder art (22 Oct 2026, git 724fec2).  Right: current build.',font=F(28,False),fill=(160,165,175))
for i,(p,t) in enumerate([('old-724fec2/class-select.png','Before'),('raw8/class-select.png','After')]):
    im=Image.open(p).convert('RGB').resize((W,H),Image.BOX); x=24+i*(W+24)
    S.paste(im,(x,head)); d.text((x,head+H+12),t,font=F(34),fill=(232,190,80))
S.save(f'{OUT}/10c-polish-class-select.png')
# 10d recruit backdrop before/after
S=Image.new('RGB',(2*W+72,head+H+lab+24),BG); d=ImageDraw.Draw(S)
d.text((24,28),'Polish: the recruitment screen',font=F(56),fill=(235,235,235))
d.text((24,96),'Same menu and rules; only the art changed.  Left: 22 Oct 2026 (git 724fec2).  Right: current build.',font=F(28,False),fill=(160,165,175))
for i,(p,t) in enumerate([('old-724fec2/recruit.png','Before'),('raw8/recruit-menu.png','After')]):
    im=Image.open(p).convert('RGB').resize((W,H),Image.BOX); x=24+i*(W+24)
    S.paste(im,(x,head)); d.text((x,head+H+12),t,font=F(34),fill=(232,190,80))
S.save(f'{OUT}/10d-polish-recruitment.png')
