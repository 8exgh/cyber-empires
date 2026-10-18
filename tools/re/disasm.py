import struct,sys,json,os,re,bisect
from iced_x86 import *
fn=sys.argv[1]; out=sys.argv[2]
d=open(fn,'rb').read()
(sig,cblp,cp,crlc,cparhdr)=struct.unpack('<5H',d[:10])
imgsize=cp*512-(512-cblp if cblp else 0)
hdr=cparhdr*16
img=d[hdr:imgsize]
dbg=d[imgsize:]
nsym=struct.unpack('<H',dbg[8:10])[0]
start=48+nsym*9
m=re.search(rb'[\x20-\x7e]{3,}\x00',dbg[start:])
pool=start+m.start()
names=['']
j=pool
while j<len(dbg):
    k=dbg.find(b'\0',j)
    if k<0: break
    names.append(dbg[j:k].decode('latin1')); j=k+1
syms=[]
for i in range(nsym):
    r=dbg[48+9*i:48+9*i+9]
    nm,ty,off,seg=struct.unpack('<4H',r[:8]); cls=r[8]
    if cls==1 or seg*16>=len(img): continue
    syms.append(dict(name=names[nm] if nm<len(names) else f'?{nm}',type=ty,off=off,seg=seg,cls=cls,addr=seg*16+off))
# DGROUP = segment with most symbols and highest seg among those <0x1000
from collections import Counter
cnt=Counter(s['seg'] for s in syms)
dgroup=[s['seg'] for s in syms if s['name']=='___FIRST'][0] if any(s['name']=='___FIRST' for s in syms) else max(cnt,key=lambda k:cnt[k])
datasyms=sorted([s for s in syms if s['seg']==dgroup and s['name']], key=lambda s:s['off'])
codesyms=sorted([s for s in syms if s['seg']!=dgroup and s['name']], key=lambda s:s['addr'])
dataoffs=[s['off'] for s in datasyms]
codeaddrs=[s['addr'] for s in codesyms]
def dataname(off):
    i=bisect.bisect_right(dataoffs,off)-1
    if i<0: return None
    s=datasyms[i]; delta=off-s['off']
    if delta>512: return None
    return s['name']+(f"+{delta}" if delta else "")
def codename(addr):
    i=bisect.bisect_right(codeaddrs,addr)-1
    if i<0: return None
    s=codesyms[i]; delta=addr-s['addr']
    return s['name']+(f"+{delta:#x}" if delta else "")
def strat(off):
    a=dgroup*16+off
    if a>=len(img): return None
    k=img.find(b'\0',a,a+80)
    if k<0: return None
    sb=img[a:k]
    if len(sb)>=2 and all(32<=c<127 or c in (10,13,9) for c in sb): return sb.decode('latin1')
    return None
json.dump(dict(dgroup=dgroup,syms=syms),open(out+'.syms.json','w'),indent=0)
fo=open(out+'.asm','w')
fo.write(f"; {fn} DGROUP={dgroup:#x} image={len(img)} bytes\n")
# write data symbol table
fo.write("; ---- DATA SYMBOLS (DGROUP offsets) ----\n")
for i,s in enumerate(datasyms):
    nxt=datasyms[i+1]['off'] if i+1<len(datasyms) else None
    size=(nxt-s['off']) if nxt else None
    a=dgroup*16+s['off']
    raw=img[a:a+min(size or 16,32)]
    st=strat(s['off'])
    fo.write(f";  {s['off']:04x} size={size} {s['name']:28s} init={raw.hex()} {('str='+repr(st)) if st else ''}\n")
    if size and size>32 and size<=4096:
        a2=dgroup*16+s['off']
        full=img[a2:a2+size]
        if any(full):
            fo.write(';      full='+full.hex()+'\n')
fo.write("; ---- CODE ----\n")
formatter=Formatter(FormatterSyntax.NASM)
formatter.hex_prefix="0x"; formatter.hex_suffix=""
formatter.space_after_operand_separator=True
for i,s in enumerate(codesyms):
    seg=s['seg']; a=s['addr']
    # end = next symbol in same segment or next segment start
    end=codesyms[i+1]['addr'] if i+1<len(codesyms) else dgroup*16
    if end<=a: continue
    code=img[a:end]
    fo.write(f"\n;=== {s['name']}  seg={seg:#x} off={s['off']:#x} addr={a:#x} len={end-a}\n")
    dec=Decoder(16,code,ip=s['off'])
    for ins in dec:
        txt=formatter.format(ins)
        ann=[]
        for k in range(ins.op_count):
            kind=ins.op_kind(k)
            if kind==OpKind.NEAR_BRANCH16:
                t=seg*16+ins.near_branch16
                n=codename(t)
                if n: ann.append(n)
            elif kind==OpKind.FAR_BRANCH16:
                t=ins.far_branch_selector*16+ins.far_branch16
                n=codename(t); ann.append(n or f"far {ins.far_branch_selector:#x}:{ins.far_branch16:#x}")
            elif kind==OpKind.MEMORY:
                if ins.memory_base==Register.NONE and ins.memory_index==Register.NONE:
                    n=dataname(ins.memory_displacement & 0xffff)
                    if n: ann.append("["+n+"]")
                elif ins.memory_base not in (Register.BP,) and ins.memory_displacement>0x40:
                    n=dataname(ins.memory_displacement & 0xffff)
                    if n: ann.append("[+"+n+"]")
            elif kind in (OpKind.IMMEDIATE16, OpKind.IMMEDIATE8TO16):
                v=ins.immediate16 if kind==OpKind.IMMEDIATE16 else (ins.immediate8to16 & 0xffff)
                if ins.mnemonic in (Mnemonic.PUSH, Mnemonic.MOV, Mnemonic.CMP, Mnemonic.ADD, Mnemonic.SUB) and v>=0x100:
                    st=strat(v)
                    n=dataname(v)
                    if st: ann.append(f"&{repr(st)}")
                    elif n and not '+' in n: ann.append("&"+n)
                    elif n and ins.mnemonic==Mnemonic.PUSH: ann.append("&"+n)
        off=ins.ip
        bs=code[off-s['off']:off-s['off']+ins.len].hex()
        fo.write(f"{off:04x}: {bs:24s} {txt:40s} {'; '+' '.join(ann) if ann else ''}\n")
fo.close()
print("wrote",out+'.asm', len(codesyms),"code syms",len(datasyms),"data syms, DGROUP",hex(dgroup))
