# Generate src/battle/tables.rs from the original executables' data segments.
import json,struct,sys
OUT=sys.argv[1]
def load(prg,symf):
    j=json.load(open(symf)); dg=j['dgroup']
    d=open(prg,'rb').read()
    (sig,cblp,cp,crlc,cparhdr)=struct.unpack('<5H',d[:10]); imgsize=cp*512-(512-cblp if cblp else 0); img=d[cparhdr*16:imgsize]
    syms={s['name']:s for s in j['syms'] if s['seg']==dg and s['name']}
    offs=sorted(s['off'] for s in syms.values())
    byoff={s['off']:s['name'] for s in syms.values()}
    def raw(name,size=None):
        s=syms[name]; o=s['off']; i=offs.index(o); n=offs[i+1] if i+1<len(offs) else o+16
        if size: n=o+size
        return img[dg*16+o:dg*16+n]
    def shorts(name,size=None):
        b=raw(name,size); return list(struct.unpack('<%dh'%(len(b)//2),b[:len(b)//2*2]))
    return raw,shorts,byoff
craw,cs,cby=load('orig/cyber-empires/cyber/CYBOR.PRG','cybor.syms.json')
araw,as_,aby=load('orig/cyber-empires/cyber/ACTION.PRG','action.syms.json')
out=[]
def emit(name,ty,vals,comment=''):
    if vals and isinstance(vals[0],list):
        out.append(f"/// {comment}\npub const {name}: [[{ty}; {len(vals[0])}]; {len(vals)}] = {vals};")
    else:
        out.append(f"/// {comment}\npub const {name}: [{ty}; {len(vals)}] = {vals};")
def rs(v): return str(v).replace('[','[').replace(']',']')
# ---- CYBOR structure tables
emit('CAPBUILD','i16',cs('_CAPBUILD',770),'Capital pattern: 5 owners x 7 rows x 11 cols (tile ids).')
emit('FAC','i16',cs('_FAC',1860),'Factory patterns (see FACP): 5 owners per size, consecutive.')
emit('FAC3','i16',cs('_FAC3',20),'(index into FAC, tile) per owner: core replaced for slot value 5.')
emit('FACP','i16',cs('_FACP',32)[:16],'Per factory size (v=3,4,5,6): cols, stride, rows, base.')
emit('ENHANCE','i16',cs('_ENHANCE',490),'Enhancement pattern: 5 owners x 7x7.')
emit('FORT','i16',cs('_FORT',132),'Fort tiles: 3 levels x 22 (h-wall, v-wall, 20 FORTPOS pieces).')
emit('FORTPOS','i16',cs('_FORTPOS',40),'Linear tile offsets (y*80+x) of the 20 fort pieces.')
emit('TURCORDS','i16',cs('_TURCORDS',192),'Turret tile (x,y) per attack direction: 4 x 12 x 2.')
emit('TURT','i8',[struct.unpack('b',bytes([b]))[0] for b in craw('_TURT',48)],'Turret type per fort level: 4 x 12 (-1 none).')
emit('BUILDPOS','i16',cs('_BUILDPOS',16),'Anchors per build slot, low byte = row, high byte = col.')
emit('BO','i16',cs('_BO',16),'Structure shift (x,y) per attack direction.')
emit('APOS','i16',cs('_APOS',16),'Attacker base tile per direction (x,y).')
emit('AOFF','i16',cs('_AOFF',16),'Attacker spacing per direction (x,y).')
emit('AP','i16',cs('_AP',160),'Defender UI positions per direction: 4 x 10 x (x,y).')
emit('BACKUP','i16',cs('_BACKUP',16),'Defender fallback offset per direction (x,y).')
emit('SW','i16',cs('_SW',32),'Spiral search steps (x,y) x 8.')
emit('LP','i16',cs('_LP',20),'Passable tile range (lo,hi) per terrain for defender placement.')
emit('WALLCITY_STRAT','u8',list(craw('_WALLCITY',180)),'Damaged-tile lookup used for pre-battle city damage (CYBOR).')
emit('SDCTRY','i16',cs('_SDCTRY',10),'Showdown country per terrain.')
emit('PL','i16',cs('_PL',40),'Defender placement limits per direction (x0,x1,y0,y1).')
# ---- ACTION tables
w=as_('_WEAPONS',176); emit('WEAPONS','i16',w,'8 weapons x 11: range, speed, damage, fire heat, impact heat, icon, reload, snd, expl, snd, snd.')
sm=araw('_STATMECH',468)
rows=[]
for t in range(13):
    r=sm[t*36:(t+1)*36]
    rows.append(list(struct.unpack('<17h',r[:34]))+[r[34],r[35]])
emit('STATMECH','i16',rows,'13 unit types x 19: feet anim, armour, speed, weapon[4], ammo[4], shots[4], cooldiv, unused, sprite size, collision size.')
# muzzle tables
names=['_TITAN_PPC','_TITAN_LMIS','_TITAN_HAUTO','_BEL_HLAS','_BEL_SMIS','_BEL_LMIS','_BEL_LAUTO','_HERC_PPC','_HERC_HLAS','_HERC_SMIS','_DRAG_LLAS','_DRAG_HFIRE','_ACHIL_HLAS','_ACHIL_LAUTO','_CROSS_LLAS','_CROSS_LMIS','_MARS_LAUTO','_MARS_PPC','_CYC_LLAS','_CYC_LAUTO','_MERC_LLAS','_HTURRET','_MTURRET','_LTURRET']
tt=as_('_TRANS_TABLES',104)
# resolve pointers
nm=[]
for p in tt:
    nm.append(aby.get(p & 0xffff,'NONE') if p else 'NONE')
idx={n:i for i,n in enumerate(names)}
emit('TRANS_TABLES','i16',[idx.get(n,-1) for n in nm],'Muzzle table index (into MUZZLE) per type*4+slot, -1 none.')
mz=[]
for n in names:
    v=as_(n); mz.append(v)
out.append("/// Muzzle offset tables: [table][shot*16 + dir*2 + (0=x,1=y)]; up to 4 shots x 8 dirs.\npub const MUZZLE: [&[i16]; %d] = [%s];"%(len(mz),', '.join('&%s'%rs(v) for v in mz)))
# animation tables
an=as_('_ANIMATION',240)
ftabs=['_TITAN_BODY','_BEL_BODY','_HERC_BODY','_ACHIL_BODY','_CROSS_BODY','_MARS_BODY','_DRAG_BODY','_CYC_BODY','_MERC_BODY','_HTURRET_BODY','_LTURRET_BODY','_MED_FEET','_LIGHT_FEET','_HLASER','_PPC','_LIGHT_LASER','_EXPLOSION','_LIGHT_AUTO','_HEAVY_AUTO','_MISSILE','_LONG_MISSILE','_MECH_EXPLOSION','_DITTY','_HLASER_EXPLOSION']
fidx={n:i for i,n in enumerate(ftabs)}
anim=[]
for i in range(30):
    ptr,n,_,k=an[i*4:i*4+4]
    name=aby.get(ptr&0xffff,'NONE')
    anim.append([fidx.get(name,-1),n,k])
emit('ANIMATION','i16',anim,'30 animations: (frame table index into FRAMES, n per direction, extra).')
fr=[as_(n) for n in ftabs]
out.append("/// Sprite frame tables referenced by ANIMATION.\npub const FRAMES: [&[i16]; %d] = [%s];"%(len(fr),', '.join('&%s'%rs(v) for v in fr)))
emit('WALL_POWER','u8',list(araw('_WALL_POWER',180)),'Tile hit points.')
emit('WALL_LISTARC','u8',list(araw('_WALL_LISTARC',180)),'Destroyed-tile replacement, arctic/desert/volcanic.')
emit('WALL_LISTCON','u8',list(araw('_WALL_LISTCON',180)),'Destroyed-tile replacement, coniferous.')
emit('WALLCITY','u8',list(araw('_WALLCITY',180)),'Destroyed-tile replacement, city.')
emit('COL','i16',as_('_COL',60),'Blocking tile ranges per terrain: 5 x 3 x (lo,hi).')
emit('PRIORITIES','i16',as_('_PRIORITIES',72),'Weapon slot preference per cyborg type: 9 x 4.')
emit('GOALTABLE','u8',list(araw('_GOALTABLE',16)),'Target weights per goal: 4 x (cyborg, capital, structure, turret).')
emit('DIR_CORRECT','i8',[struct.unpack('b',bytes([b]))[0] for b in araw('_DIR_CORRECT',8)],'Default avoidance turn per facing.')
emit('D1','i8',[struct.unpack('b',bytes([b]))[0] for b in araw('_D1',16)],'')
emit('D2','i8',[struct.unpack('b',bytes([b]))[0] for b in araw('_D2',16)],'')
emit('DIRTAB','i16',as_('_DIRTAB',18),'(sign dy+1)*3 + sign dx+1 -> direction.')
emit('DIR','i16',as_('_DIR',32),'Unit step per direction (x,y).')
emit('FIRESTUF','i16',as_('_FIRESTUF',32),'Bullet velocity basis per direction (x,y) /10.')
emit('PRIOR','u8',list(araw('_PRIOR',5)),'Aggression multiplier.')
emit('JSREF','i16',as_('_JSREF',18)[:9],'Joystick quadrant -> direction.')
emit('TERRAIN_COLOURS','u16',[x&0xffff for x in as_('_TERRAIN_COLOURS',50)],'5 terrains x 5 ST colours (12-bit).')
emit('COLOUR_PALETTE','u16',[x&0xffff for x in as_('_COLOUR_PALETTE',32)],'Base HUD colours (12-bit).')
emit('PLAYER_COLOURS','u16',[x&0xffff for x in as_('_PLAYER_COLOURS',30)],'5 players x 3 colours (12-bit).')
emit('TOFF','i16',as_('_TOFF',10),'Palette slots for terrain colours.')
emit('P1OFF','i16',as_('_P1OFF',6),'Palette slots for side 0 colours.')
emit('P2OFF','i16',as_('_P2OFF',6),'Palette slots for side 1 colours.')
open(OUT,'w').write("//! Data tables extracted from the original CYBOR.PRG / ACTION.PRG (generated by tools/re/gen_tables.py).\n#![allow(clippy::all)]\n\n"+"\n\n".join(out)+"\n")
print("wrote",OUT, sum(len(o) for o in out),"bytes")
