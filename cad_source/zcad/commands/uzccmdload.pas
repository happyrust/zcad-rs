{
*****************************************************************************
*                                                                           *
*  This file is part of the ZCAD                                            *
*                                                                           *
*  See the file COPYING.txt, included in this distribution,                 *
*  for details about the copyright.                                         *
*                                                                           *
*  This program is distributed in the hope that it will be useful,          *
*  but WITHOUT ANY WARRANTY; without even the implied warranty of           *
*  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.                     *
*                                                                           *
*****************************************************************************
}
{
@author(Andrey Zubarev <zamtmn@yandex.ru>) 
}

unit uzccmdload;
{$INCLUDE zengineconfig.inc}

interface

uses
  uzcLog,LCLType,LazUTF8,
  uzbpaths,uzbtypes,uzcuitypes,
  uzeffmanager,uzctranslations,
  uzccommandsimpl,uzccommandsabstract,
  uzcdrawings,uzcdrawing,
  uzctnrVectorBytes,UUnitManager,URecordDescriptor,gzctnrVectorTypes,
  Varman,varmandef,typedescriptors,
  uzgldrawcontext,
  uzedrawingsimple,uzeconsts,
  uzcinterface,
  uzcstrconsts,
  uzcutils,
  SysUtils,
  uzelongprocesssupport,uzccommandsmanager,
  uzcreglog,uzeLogIntf;

{ 加载并合并文件的命令入口
  Operands: 命令操作数（通常为文件路径）
  LoadMode: 加载模式（如合并/替换等）
  返回值: 命令执行结果 }
function Load_Merge(const Operands:TCommandOperands;LoadMode:TLoadOpt):TCommandResult;

{ 内部实现：根据提供的加载过程指针与加载模式，加载文件并合并到当前图纸
  s: 文件名（UTF-8）
  loadproc: 实际文件加载过程（由扩展到加载器的映射获取）
  LoadMode: 加载选项
  返回值: 命令执行结果 }
function Internal_Load_Merge(const s:ansistring;loadproc:TFileLoadProcedure;
  LoadMode:TLoadOpt):TCommandResult;

{ DXF 加载过程中的回调，用于输出不同阶段/类型的日志信息
  stage: 当前阶段
  &Type: 消息类型（信息/警告/错误）
  msg: 文本消息 }
procedure DXFLoadCallBack(stage:TZEStage;&Type:TZEMsgType;msg:string);

implementation

{ 重新映射项目数据库指针
  目的：将已加载单元中的对象实例中的 Variants 字段指向 DBUnit 对应位置
  pu: 需要重映射的单元 }
procedure remapprjdb(pu:ptunit);
var
  pv,pvindb:pvardesk;
  ir:itrec;
  ptd:PUserTypeDescriptor;
  pfd:PFieldDescriptor;
  pf,pfindb:ppointer;
begin
  pv:=pu.InterfaceVariables.vardescarray.beginiterate(ir);
  if pv<>nil then
    repeat
      // 根据类型名在 DBUnit 中找到对应的类型描述符
      ptd:=DBUnit.TypeName2PTD(pv.Data.PTD.TypeName);
      if ptd<>nil then
        // 仅处理对象类型
        if (ptd.GetTypeAttributes and TA_OBJECT)=TA_OBJECT then begin
          // 在 DBUnit 的接口变量中找到相同类型的变量描述
          pvindb:=DBUnit.InterfaceVariables.findvardescbytype(pv.Data.PTD);
          if pvindb<>nil then begin
            // 查找名为 'Variants' 的字段
            pfd:=PRecordDescriptor(pvindb^.Data.PTD)^.FindField('Variants');
            if pfd<>nil then begin
              // 取出 Variants 字段地址，并将其指针指向 DBUnit 中对应字段
              pf:=pv.Data.Addr.Instance+pfd.Offset;
              pfindb:=pvindb.Data.Addr.Instance+pfd.Offset;
              pf^:=pfindb^;
            end;
          end;
        end;
      pv:=pu.InterfaceVariables.vardescarray.iterate(ir);
    until pv=nil;
end;

{ DXF 加载回调：根据命令管理器是否忙碌，决定日志的显示方式（静默/弹窗） }
procedure DXFLoadCallBack(stage:TZEStage;&Type:TZEMsgType;msg:string);
begin
  if commandmanager.isBusy then begin
    // 命令正在执行：仅记录日志或静默提示
    case &Type of
      ZEMsgInfo:ProgramLog.LogOutStr(msg,LM_Info);
      ZEMsgCriticalInfo:ProgramLog.LogOutStr(msg,LM_Info,1,MO_SH);
      ZEMsgWarning:ProgramLog.LogOutStr(msg,LM_Info);
      ZEMsgError:ProgramLog.LogOutStr(msg,LM_Info,1,MO_SH);
    end;
  end else begin
    // 非忙碌状态：重要信息可提示到界面
    case &Type of
      ZEMsgInfo:ProgramLog.LogOutStr(msg,LM_Info,1,MO_SH);
      ZEMsgCriticalInfo:ProgramLog.LogOutStr(msg,LM_Info,1,MO_SH);
      ZEMsgWarning:ProgramLog.LogOutStr(msg,LM_Info,1,MO_SH);
      ZEMsgError:ProgramLog.LogOutStr(msg,LM_Info,1,MO_SM);
    end;
    //zDebugLn(msg);
  end;
end;

{ 内部加载与合并实现流程：
  1) 开启长任务提示
  2) 构造绘图上下文并调用具体加载过程
  3) 尝试加载同名 .dbpas 数据并进行单元映射
  4) 重新构建对象树、计算包围盒并格式化实体
  5) 完成后刷新视图并结束长任务 }
function Internal_Load_Merge(const s:ansistring;loadproc:TFileLoadProcedure;
  LoadMode:TLoadOpt):TCommandResult;
var
  mem:TZctnrVectorBytes;      // 用于读取 .dbpas 的内存容器
  pu:ptunit;                  // 单元指针（设备/库等）
  DC:TDrawContext;            // 绘制上下文
  ZCDCtx:TZDrawingContext;    // ZCAD 加载上下文
  lph,lph2:TLPSHandle;        // 长任务句柄
  dbpas:string;               // 伴随数据文件路径（.dbpas）
begin
  // 开始整体加载长任务
  lph:=lps.StartLongProcess(rsLoadFile,nil,0);

  // 构造加载上下文并调用具体文件加载器
  ZCDCtx.CreateRec(drawings.GetCurrentDWG^,drawings.GetCurrentDWG^.pObjRoot^,
    loadmode,drawings.GetCurrentDWG.CreateDrawingRC);
  loadproc(s,ZCDCtx,@DXFLoadCallBack);

  // 尝试定位 .dbpas 伴随数据文件（同名或追加后缀）
  dbpas:=utf8tosys(ChangeFileExt(s,'.dbpas'));
  if not FileExists(dbpas) then begin
    dbpas:=utf8tosys(s+'.dbpas');
    if not FileExists(dbpas) then
      dbpas:='';
  end;

  // 如果存在 .dbpas，则解析并将其映射到当前 DWG 的支持单元
  if dbpas<>'' then begin
    pu:=PTZCADDrawing(drawings.GetCurrentDWG).DWGUnits.findunit(
      GetSupportPaths,InterfaceTranslate,DrawingDeviceBaseUnitName);
    if assigned(pu) then begin
      mem.InitFromFile(dbpas);
      units.parseunit(GetSupportPaths,InterfaceTranslate,mem,PTSimpleUnit(pu));
      remapprjdb(pu);
      mem.done;
    end;
  end;

  // 构造绘图 RC（Render Context）
  dc:=drawings.GetCurrentDWG^.CreateDrawingRC;

  // 第一次：在 DXF 加载后构建树并计算包围盒（静默）
  lph2:=lps.StartLongProcess('First maketreefrom afrer dxf load',nil,0,LPSOSilent);
  drawings.GetCurrentROOT.calcbb(dc);
  drawings.GetCurrentDWG^.pObjRoot.ObjArray.ObjTree.maketreefrom(
    drawings.GetCurrentDWG^.pObjRoot.ObjArray,
    drawings.GetCurrentDWG^.pObjRoot.vp.BoundingBox,nil);
  lps.EndLongProcess(lph2);

  // 格式化实体（例如应用样式、文本格式等）（静默）
  lph2:=lps.StartLongProcess('drawings.GetCurrentROOT.FormatEntity afrer dxf load',
    nil,0,LPSOSilent);
  drawings.GetCurrentROOT.FormatEntity(drawings.GetCurrentDWG^,dc);
  lps.EndLongProcess(lph2);

  // 第二次：再次构建树并重绘（避免 BlockBaseDWG 的特殊情况）
  lph2:=lps.StartLongProcess('Second maketreefrom and redraw afrer dxf load',
    nil,0,LPSOSilent);
  if drawings.currentdwg<>PTSimpleDrawing(BlockBaseDWG) then begin
    drawings.GetCurrentDWG^.pObjRoot.ObjArray.ObjTree.maketreefrom(
      drawings.GetCurrentDWG^.pObjRoot.ObjArray,
      drawings.GetCurrentDWG^.pObjRoot.vp.BoundingBox,nil);
    zcRedrawCurrentDrawing;
  end;
  lps.EndLongProcess(lph2);

  // 结束整体加载长任务
  lps.EndLongProcess(lph);

  // 通知 UI 进行重绘
  zcUI.Do_GUIaction(nil,zcMsgUIActionRedraw);

  Result:=cmd_ok;
end;

{ 命令入口：加载并合并文件到当前图纸
  1) 若当前图纸非 BlockBase 且已含数据，提示用户是否继续
  2) 根据文件扩展名获取加载过程，并检查文件是否存在
  3) 调用内部实现进行加载，否则给出错误消息 }
function Load_Merge(const Operands:TCommandOperands;LoadMode:TLoadOpt):TCommandResult;
var
  s:ansistring;               // 文件路径（命令操作数）
  isload:boolean;             // 是否可加载
  loadproc:TFileLoadProcedure;// 文件加载过程
begin
  // 非块库底图且当前图纸已有对象，提示用户
  if drawings.currentdwg<>PTSimpleDrawing(BlockBaseDWG) then
    if drawings.GetCurrentROOT.ObjArray.Count>0 then begin
      if zcUI.TextQuestion(rsDWGAlreadyContainsData,'QLOAD')=zccbNo then
        exit;
    end;

  // 解析操作数字符串为文件名
  s:=operands;

  // 根据扩展名获取加载过程，并检查文件存在性
  loadproc:=Ext2LoadProcMap.GetLoadProc(extractfileext(s));
  isload:=(assigned(loadproc))and(FileExists(utf8tosys(s)));

  // 执行加载或报错
  if isload then begin
    Result:=Internal_Load_Merge(s,loadproc,LoadMode);
  end else
    zcUI.TextMessage('MERGE:'+format(rsUnableToOpenFile,[s]),TMWOShowError);
end;

end.
